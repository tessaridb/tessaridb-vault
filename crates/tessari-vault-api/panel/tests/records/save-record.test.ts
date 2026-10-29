// Making a record in the browser: a fresh id chosen here, revision 1 sealed and signed over that id, the record key
// wrapped to the author's own key, and the reader opens exactly what was made.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { decode } from "../../src/base64url.ts";
import { hex, random } from "../../src/crypto/bytes.ts";
import type { OpenKeySet } from "../../src/crypto/key-set.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Statement } from "../../src/crypto/statements.ts";
import type { RecordDocument } from "../../src/records/document.ts";
import { openRecord } from "../../src/records/open-record.ts";
import { memberKey } from "../../src/records/recipient-keys.ts";
import { newRecord, revisedRecord } from "../../src/records/save-record.ts";
import { readOpened } from "../../src/records-model.ts";
import { refusedWith } from "../crypto/support.ts";

const COMPANY = "company-1";
const DOCUMENT: RecordDocument = {
  v: 1,
  type: "login",
  title: "Mail admin",
  sections: [{ name: "", fields: [{ type: "concealed", label: "Password", value: "hunter2" }] }],
};

async function author(): Promise<{ keys: OpenKeySet; account: string }> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  const signingPublic = await publicBytes(signing.publicKey);
  const keys: OpenKeySet = {
    keyId: random(16),
    agreementPrivate: agreement.privateKey,
    signingPrivate: signing.privateKey,
    agreementPublic: await publicBytes(agreement.publicKey),
    signingPublic,
    ownerSigningPublic: signingPublic,
  };
  return { keys, account: "a".repeat(32) };
}

describe("making a record", () => {
  it("seals revision 1 under a fresh id that the reader opens back, as its author", async () => {
    const { keys, account } = await author();
    const made = await newRecord(COMPANY, keys, DOCUMENT);
    assert.match(made.id, /^[0-9a-f]{32}$/);
    assert.equal(made.body["id"], made.id);
    assert.equal(made.body["record_type"], "login");
    assert.equal(made.body["author_key"], hex(keys.keyId));
    const genesis = await appendStatement(
      COMPANY,
      undefined,
      { kind: "member", subject: account, agreementPublic: keys.agreementPublic, signingPublic: keys.signingPublic },
      { privateKey: keys.signingPrivate, publicBytes: keys.signingPublic },
    );
    const vouched = await verifyChain(COMPANY, keys.signingPublic, [genesis]);
    const row = { id: made.id, record_type: "login", revision: 1, key_id: made.body["key_id"], author: account, created: 1, updated: 1, rotation_due: false };
    const opened = readOpened({ record: row, body: made.body["body"], signature: made.body["signature"], author_key: made.body["author_key"], recipient: `member:${hex(keys.keyId)}`, wrap: made.body["wrap"], through: [], revised_by: row.author });
    assert.notEqual(opened, null);
    if (opened !== null) {
      assert.deepEqual(await openRecord(COMPANY, vouched, memberKey(keys), opened), DOCUMENT);
    }
  });

  it("chooses a different id and record key every time, and never sends the plaintext", async () => {
    const { keys } = await author();
    const [first, second] = [await newRecord(COMPANY, keys, DOCUMENT), await newRecord(COMPANY, keys, DOCUMENT)];
    assert.notEqual(first.id, second.id);
    assert.notEqual(first.body["key_id"], second.body["key_id"]);
    const sent = JSON.stringify(first.body);
    assert.doesNotMatch(sent, /hunter2|Mail admin/);
    assert.doesNotMatch(Buffer.from(decode(String(first.body["body"])) ?? []).toString("latin1"), /hunter2|Mail admin/);
  });
});

describe("revising a record", () => {
  it("seals the next revision under a new key, wrapped to its author and to every vouched share recipient", async () => {
    const { keys, account } = await author();
    const reader = await author();
    const statements: Statement[] = [];
    for (const [who, subject] of [[keys, account], [reader.keys, "b".repeat(32)]] as const) {
      statements.push(
        await appendStatement(
          COMPANY,
          statements.at(-1),
          { kind: "member", subject, agreementPublic: who.agreementPublic, signingPublic: who.signingPublic },
          { privateKey: keys.signingPrivate, publicBytes: keys.signingPublic },
        ),
      );
    }
    const vouched = await verifyChain(COMPANY, keys.signingPublic, statements);
    const row = { id: "c".repeat(32), recordType: "login", revision: 2, keyId: "d".repeat(32), author: account, created: 1, updated: 2, rotationDue: false };
    const changed: RecordDocument = { ...DOCUMENT, title: "Mail admin (new)" };
    const share = { recipient: { kind: "member" as const, account: "b".repeat(32) }, keyId: reader.keys.keyId, listed: reader.keys.agreementPublic };
    const body = await revisedRecord(COMPANY, keys, vouched, row, changed, [share]);
    assert.equal(body["revision"], 2, "the revision it was read at");
    assert.notEqual(body["key_id"], row.keyId, "a new record key");
    const wraps = body["wraps"] as { recipient: string; wrap: string }[];
    assert.deepEqual(wraps.map((each) => each.recipient), [`member:${hex(reader.keys.keyId)}`]);
    const next = { id: row.id, record_type: "login", revision: 3, key_id: body["key_id"], author: account, created: 1, updated: 3, rotation_due: false };
    const parts = { record: next, body: body["body"], signature: body["signature"], author_key: body["author_key"], through: [], revised_by: row.author };
    const forReader = readOpened({ ...parts, recipient: `member:${hex(reader.keys.keyId)}`, wrap: wraps[0]?.wrap });
    const forAuthor = readOpened({ ...parts, recipient: `member:${hex(keys.keyId)}`, wrap: body["wrap"] });
    assert.ok(forReader !== null && forAuthor !== null);
    assert.deepEqual(await openRecord(COMPANY, vouched, memberKey(reader.keys), forReader), changed);
    assert.deepEqual(await openRecord(COMPANY, vouched, memberKey(keys), forAuthor), changed);
  });

  it("refuses to wrap to a share recipient key the chain does not vouch for", async () => {
    const { keys, account } = await author();
    const genesis = await appendStatement(
      COMPANY,
      undefined,
      { kind: "member", subject: account, agreementPublic: keys.agreementPublic, signingPublic: keys.signingPublic },
      { privateKey: keys.signingPrivate, publicBytes: keys.signingPublic },
    );
    const vouched = await verifyChain(COMPANY, keys.signingPublic, [genesis]);
    const row = { id: "c".repeat(32), recordType: "login", revision: 1, keyId: "d".repeat(32), author: account, created: 1, updated: 1, rotationDue: false };
    const stranger = { recipient: { kind: "member" as const, account: "e".repeat(32) }, keyId: random(16), listed: (await author()).keys.agreementPublic };
    await refusedWith(revisedRecord(COMPANY, keys, vouched, row, DOCUMENT, [stranger]), "unvouched-recipient");
  });
});
