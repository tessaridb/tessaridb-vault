// Making and revising a record in the browser. The record id is chosen here, because revision 1 is sealed and signed
// over it before the record exists (Q-VA-39). Every revision gets a new record key: the body is sealed under it, the
// author signs the revision, and the key is wrapped to the author's own key and, for a revision, to every recipient the
// record is shared with, each only when the owner's chain vouches for the key the server listed. Only the sealed parts
// leave the page.

import { encode } from "../base64url.ts";
import { hex, random, wipe, type Bytes } from "../crypto/bytes.ts";
import type { OpenKeySet } from "../crypto/key-set.ts";
import { importRecordKey, sealRevision, signRevision } from "../crypto/record.ts";
import { wrapToVouched, type Recipient } from "../crypto/recipients.ts";
import type { Vouched } from "../crypto/statements.ts";
import { wrapSecret } from "../crypto/wraps.ts";
import type { RecordRow } from "../records-model.ts";
import { encodeRecord, type RecordDocument } from "./document.ts";

const ID_BYTES = 16;
const RECORD_KEY_BYTES = 32;

/** A recipient the record is shared with: who, their key's id, and the public key the server lists for it. */
export interface ShareRecipient {
  readonly recipient: Exclude<Recipient, { kind: "recovery" }>;
  readonly keyId: Bytes;
  readonly listed: Bytes;
}

type Body = Readonly<Record<string, unknown>>;

/**
 * Revision `revision` of record `id` sealed under a new key, signed at chain position `chainSeq` (the seq of the last
 * statement of the chain the author verified), and wrapped to the author and to `others`.
 */
async function sealed(company: string, keys: OpenKeySet, id: string, revision: number, chainSeq: number, document: RecordDocument, vouched: Vouched | null, others: readonly ShareRecipient[]) {
  const keyId = random(ID_BYTES);
  const raw = random(RECORD_KEY_BYTES);
  const context = (recipientKind: "member" | "group", recipientKeyId: Bytes) =>
    ({ company, wrappedKind: "record", wrappedKeyId: keyId, recipientKind, recipientKeyId }) as const;
  const copy = raw.slice();
  let wrap: Bytes;
  const wraps: { recipient: string; wrap: string }[] = [];
  try {
    wrap = await wrapSecret(keys.agreementPublic, context("member", keys.keyId), copy);
    for (const other of others) {
      if (vouched === null) {
        throw new RangeError("a revision shared with others needs the verified chain");
      }
      const made = await wrapToVouched(vouched, other.recipient, other.listed, context(other.recipient.kind, other.keyId), copy);
      wraps.push({ recipient: `${other.recipient.kind}:${hex(other.keyId)}`, wrap: encode(made) });
    }
  } finally {
    wipe(copy);
  }
  const place = { company, record: id, revision };
  const body = await sealRevision(await importRecordKey(raw), { ...place, keyId }, encodeRecord(document));
  const signature = await signRevision(keys.signingPrivate, { ...place, chainSeq }, body);
  return { key_id: hex(keyId), body: encode(body), signature: encode(signature), author_key: hex(keys.keyId), chain_seq: chainSeq, wrap: encode(wrap), wraps };
}

/** A new record of `document`'s type, signed at `chainSeq`: its id, and the body `POST /api/companies/{company}/records` takes. */
export async function newRecord(company: string, keys: OpenKeySet, chainSeq: number, document: RecordDocument): Promise<{ readonly id: string; readonly body: Body }> {
  const id = hex(random(ID_BYTES));
  const { wraps: _none, ...parts } = await sealed(company, keys, id, 1, chainSeq, document, null, []);
  return { id, body: { id, record_type: document.type, ...parts } };
}

/** The body `PUT /api/companies/{company}/records/{record}` takes for the revision after `row`, shared as before. */
export async function revisedRecord(
  company: string,
  keys: OpenKeySet,
  vouched: Vouched,
  row: RecordRow,
  document: RecordDocument,
  others: readonly ShareRecipient[],
): Promise<Body> {
  return { revision: row.revision, ...(await sealed(company, keys, row.id, row.revision + 1, vouched.length, document, vouched, others)) };
}
