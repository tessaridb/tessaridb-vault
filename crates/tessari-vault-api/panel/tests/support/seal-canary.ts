// Seals one plaintext through the browser's own crypto module and prints the create-record body the browser would
// send, for the live suite's raw scan (C2): node tests/support/seal-canary.ts <company> <author key id> <plaintext>.

import { utf8 } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { importRecordKey, sealRevision, signRevision } from "../../src/crypto/record.ts";
import { wrapSecret } from "../../src/crypto/wraps.ts";

const [company, authorKey, plaintext] = process.argv.slice(2);
if (company === undefined || authorKey === undefined || plaintext === undefined) {
  throw new Error("usage: seal-canary.ts <company> <author key id> <plaintext>");
}
const base64url = (bytes: Uint8Array): string => Buffer.from(bytes).toString("base64url");
const fromHex = (text: string): Uint8Array<ArrayBuffer> => new Uint8Array(Buffer.from(text, "hex"));

const rawKey = crypto.getRandomValues(new Uint8Array(32));
const keyId = crypto.getRandomValues(new Uint8Array(16));
// The browser chooses the record id and seals revision 1 over it.
const record = Buffer.from(crypto.getRandomValues(new Uint8Array(16))).toString("hex");
const context = { company, record, revision: 1 };
const body = await sealRevision(await importRecordKey(rawKey), { ...context, keyId }, utf8(plaintext));
// The canary's company has no key chain: position 0, which the server takes and no browser verifies.
const chainSeq = 0;
const signing = await generatePair("Ed25519");
const signature = await signRevision(signing.privateKey, { ...context, chainSeq }, body);
const agreement = await generatePair("X25519");
const wrap = await wrapSecret(
  await publicBytes(agreement.publicKey),
  { company, wrappedKind: "record", wrappedKeyId: keyId, recipientKind: "member", recipientKeyId: fromHex(authorKey) },
  rawKey,
);
rawKey.fill(0);
process.stdout.write(
  JSON.stringify({
    id: record,
    record_type: "login",
    key_id: Buffer.from(keyId).toString("hex"),
    body: base64url(body),
    signature: base64url(signature),
    author_key: authorKey,
    chain_seq: chainSeq,
    wrap: base64url(wrap),
  }),
);
