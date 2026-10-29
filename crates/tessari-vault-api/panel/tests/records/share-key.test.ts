// The record key a share wraps: taken out of the author's own wrap once the author's signature checks out, it opens
// the record for the member it is wrapped to, and a revision whose signature does not check out yields no key.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { decode, encode } from "../../src/base64url.ts";
import { hex, random } from "../../src/crypto/bytes.ts";
import type { OpenKeySet } from "../../src/crypto/key-set.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Statement } from "../../src/crypto/statements.ts";
import type { RecordDocument } from "../../src/records/document.ts";
import { openRecord, recordKeyOf } from "../../src/records/open-record.ts";
import { shareBody } from "../../src/records/share-record.ts";
import { memberKey } from "../../src/records/recipient-keys.ts";
import { newRecord } from "../../src/records/save-record.ts";
import { readOpened } from "../../src/records-model.ts";
import { flipped, refusedWith } from "../crypto/support.ts";

const COMPANY = "company-1";
const DOCUMENT: RecordDocument = { v: 1, type: "login", title: "Mail admin", sections: [{ name: "", fields: [{ type: "concealed", label: "Password", value: "hunter2" }] }] };

async function member(): Promise<OpenKeySet> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  const signingPublic = await publicBytes(signing.publicKey);
  return {
    keyId: random(16),
    agreementPrivate: agreement.privateKey,
    signingPrivate: signing.privateKey,
    agreementPublic: await publicBytes(agreement.publicKey),
    signingPublic,
    ownerSigningPublic: signingPublic,
  };
}

describe("the record key a share wraps", () => {
  it("opens the record for the member it is wrapped to, once the author's signature checks out", async () => {
    const [author, reader] = [await member(), await member()];
    const [authorAccount, readerAccount] = ["a".repeat(32), "b".repeat(32)];
    const statements: Statement[] = [];
    for (const [keys, subject] of [[author, authorAccount], [reader, readerAccount]] as const) {
      statements.push(
        await appendStatement(COMPANY, statements.at(-1), { kind: "member", subject, agreementPublic: keys.agreementPublic, signingPublic: keys.signingPublic }, {
          privateKey: author.signingPrivate,
          publicBytes: author.signingPublic,
        }),
      );
    }
    const vouched = await verifyChain(COMPANY, author.signingPublic, statements);
    const made = await newRecord(COMPANY, author, DOCUMENT);
    const row = { id: made.id, record_type: "login", revision: 1, key_id: made.body["key_id"], author: authorAccount, created: 1, updated: 1, rotation_due: false };
    const parts = { record: row, body: made.body["body"], signature: made.body["signature"], author_key: made.body["author_key"], through: [], revised_by: row.author };
    const asAuthor = readOpened({ ...parts, recipient: `member:${hex(author.keyId)}`, wrap: made.body["wrap"] });
    assert.ok(asAuthor !== null);

    const body = await shareBody(COMPANY, vouched, memberKey(author), asAuthor, {
      recipient: { kind: "member", account: readerAccount },
      keyId: reader.keyId,
      listed: reader.agreementPublic,
    }, "edit");
    assert.deepEqual({ ...body, wrap: undefined }, { member: readerAccount, right: "edit", key_id: hex(reader.keyId), wrap: undefined });
    const wrap = decode(String(body["wrap"])) ?? new Uint8Array();
    const asReader = readOpened({ ...parts, recipient: `member:${hex(reader.keyId)}`, wrap: encode(wrap) });
    assert.ok(asReader !== null);
    assert.deepEqual(await openRecord(COMPANY, vouched, memberKey(reader), asReader), DOCUMENT);

    const forged = readOpened({ ...parts, signature: encode(flipped(decode(String(made.body["signature"])) ?? new Uint8Array(65), 40)), recipient: `member:${hex(author.keyId)}`, wrap: made.body["wrap"] });
    assert.ok(forged !== null);
    await refusedWith(recordKeyOf(COMPANY, vouched, memberKey(author), forged), "bad-signature");
  });
});
