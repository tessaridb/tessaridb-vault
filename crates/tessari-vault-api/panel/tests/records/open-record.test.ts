// Opening a record in the browser: the revision's author must be vouched for by the owner's chain and must have signed
// exactly this place and body, the record key comes out of the wrap kept for the reader's own key or a group key they
// hold, and only then is the body opened and read as a record.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { hex, random, type Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { importRecordKey, sealRevision, signRevision } from "../../src/crypto/record.ts";
import { appendStatement, verifyChain, type Signer, type Statement, type StatementFields } from "../../src/crypto/statements.ts";
import { wrapSecret } from "../../src/crypto/wraps.ts";
import { openRecord, type RecipientKey } from "../../src/records/open-record.ts";
import { encodeRecord, type RecordDocument } from "../../src/records/document.ts";
import type { OpenedRecord } from "../../src/records-model.ts";
import { refusedWith } from "../crypto/support.ts";

const COMPANY = "company-1";
const RECORD = "aa".repeat(16);
const DOCUMENT: RecordDocument = {
  v: 1,
  type: "login",
  title: "Mail admin",
  sections: [{ name: "", fields: [{ type: "text", label: "username", value: "admin" }] }],
};

interface Person {
  signer: Signer;
  agreement: CryptoKeyPair;
  agreementPublic: Bytes;
}

async function person(): Promise<Person> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  return {
    signer: { privateKey: signing.privateKey, publicBytes: await publicBytes(signing.publicKey) },
    agreement,
    agreementPublic: await publicBytes(agreement.publicKey),
  };
}

const memberFields = (account: string, who: Person): StatementFields => ({
  kind: "member",
  subject: account,
  agreementPublic: who.agreementPublic,
  signingPublic: who.signer.publicBytes,
});

/** The owner's chain vouching for alice (who writes) and bob (who reads), plus a group key; `extra` statements after. */
async function world(extra: (people: { mallory: Person }) => StatementFields[] = () => []) {
  const [owner, alice, bob, mallory] = [await person(), await person(), await person(), await person()];
  const team = await generatePair("X25519");
  const teamPublic = await publicBytes(team.publicKey);
  const statements: Statement[] = [];
  const fields = [
    memberFields("owner", owner),
    memberFields("alice", alice),
    memberFields("bob", bob),
    { kind: "group" as const, subject: "team-1", agreementPublic: teamPublic, signingPublic: new Uint8Array(0) },
    ...extra({ mallory }),
  ];
  for (const each of fields) {
    statements.push(await appendStatement(COMPANY, statements.at(-1), each, owner.signer));
  }
  const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, statements);
  const bobKey: RecipientKey = { kind: "member", keyId: random(16), agreementPrivate: bob.agreement.privateKey, agreementPublic: bob.agreementPublic };
  const teamKey: RecipientKey = { kind: "group", keyId: random(16), agreementPrivate: team.privateKey, agreementPublic: teamPublic };
  return { alice, mallory, vouched, bobKey, teamKey };
}

/** A revision `author` sealed and signed, wrapped for `reader`, as the opened page carries it. */
async function sealed(author: Person, reader: RecipientKey, change: { author?: string; revision?: number } = {}): Promise<OpenedRecord> {
  const keyId = random(16);
  const raw = random(32);
  const wrap = await wrapSecret(
    reader.agreementPublic,
    { company: COMPANY, wrappedKind: "record", wrappedKeyId: keyId, recipientKind: reader.kind, recipientKeyId: reader.keyId },
    raw.slice(),
  );
  const recordKey = await importRecordKey(raw);
  const body = await sealRevision(recordKey, { company: COMPANY, record: RECORD, revision: 2, keyId }, encodeRecord(DOCUMENT));
  const signature = await signRevision(author.signer.privateKey, { company: COMPANY, record: RECORD, revision: 2 }, body);
  const row = {
    id: RECORD,
    recordType: "login",
    revision: change.revision ?? 2,
    keyId: hex(keyId),
    author: change.author ?? "alice",
    created: 1,
    updated: 2,
    rotationDue: false,
  };
  return { row, body, signature, authorKey: "bb".repeat(16), recipient: { kind: reader.kind, keyId: hex(reader.keyId) }, wrap };
}

describe("opening a record", () => {
  it("opens a revision wrapped to the reader's own key, and one wrapped to a group key they hold", async () => {
    const { alice, vouched, bobKey, teamKey } = await world();
    assert.deepEqual(await openRecord(COMPANY, vouched, bobKey, await sealed(alice, bobKey)), DOCUMENT);
    assert.deepEqual(await openRecord(COMPANY, vouched, teamKey, await sealed(alice, teamKey)), DOCUMENT);
  });

  it("refuses an author the chain does not vouch for, or one the server names in place of the signer", async () => {
    const { alice, mallory, vouched, bobKey } = await world();
    await refusedWith(openRecord(COMPANY, vouched, bobKey, await sealed(mallory, bobKey, { author: "mallory" })), "unvouched-signer");
    await refusedWith(openRecord(COMPANY, vouched, bobKey, await sealed(mallory, bobKey)), "bad-signature");
    await refusedWith(openRecord(COMPANY, vouched, bobKey, await sealed(alice, bobKey, { author: "bob" })), "bad-signature");
  });

  it("refuses a revision moved to another place or company", async () => {
    const { alice, vouched, bobKey } = await world();
    await refusedWith(openRecord(COMPANY, vouched, bobKey, await sealed(alice, bobKey, { revision: 3 })), "bad-signature");
    await refusedWith(openRecord("company-2", vouched, bobKey, await sealed(alice, bobKey)), "bad-signature");
  });

  it("refuses a wrap kept for a key other than the one offered", async () => {
    const { alice, vouched, bobKey, teamKey } = await world();
    const forTeam = await sealed(alice, teamKey);
    await refusedWith(openRecord(COMPANY, vouched, bobKey, forTeam), "key-mismatch");
    await refusedWith(openRecord(COMPANY, vouched, { ...teamKey, kind: "member" }, forTeam), "key-mismatch");
    const relabelled = { ...forTeam, recipient: { kind: "member" as const, keyId: forTeam.recipient.keyId } };
    await refusedWith(openRecord(COMPANY, vouched, { ...teamKey, kind: "member" }, relabelled), "unauthentic");
  });
});
