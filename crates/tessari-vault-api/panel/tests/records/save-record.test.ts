// Making a record in the browser: a fresh id chosen here, revision 1 sealed and signed over that id, the record key
// wrapped to the author's own key, and the reader opens exactly what was made.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { decode } from "../../src/base64url.ts";
import { hex, random } from "../../src/crypto/bytes.ts";
import type { OpenKeySet } from "../../src/crypto/key-set.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain } from "../../src/crypto/statements.ts";
import type { RecordDocument } from "../../src/records/document.ts";
import { openRecord } from "../../src/records/open-record.ts";
import { memberKey } from "../../src/records/recipient-keys.ts";
import { newRecord } from "../../src/records/save-record.ts";
import { readOpened } from "../../src/records-model.ts";

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
    const opened = readOpened({ record: row, body: made.body["body"], signature: made.body["signature"], author_key: made.body["author_key"], recipient: `member:${hex(keys.keyId)}`, wrap: made.body["wrap"] });
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
