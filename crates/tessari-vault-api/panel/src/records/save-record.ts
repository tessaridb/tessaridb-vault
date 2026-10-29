// Making a record in the browser. The record id is chosen here, because revision 1 is sealed and signed over it before
// the record exists (Q-VA-39); a new record key seals the body, the author signs the revision, and the key is wrapped
// to the author's own key. Only the sealed parts leave the page.

import { encode } from "../base64url.ts";
import { hex, random, wipe } from "../crypto/bytes.ts";
import type { OpenKeySet } from "../crypto/key-set.ts";
import { importRecordKey, sealRevision, signRevision } from "../crypto/record.ts";
import { wrapSecret } from "../crypto/wraps.ts";
import { encodeRecord, type RecordDocument } from "./document.ts";

const ID_BYTES = 16;
const RECORD_KEY_BYTES = 32;

/** A new record of `document`'s type: its id, and the body `POST /api/companies/{company}/records` takes. */
export async function newRecord(
  company: string,
  keys: OpenKeySet,
  document: RecordDocument,
): Promise<{ readonly id: string; readonly body: Readonly<Record<string, unknown>> }> {
  const id = hex(random(ID_BYTES));
  const keyId = random(ID_BYTES);
  const raw = random(RECORD_KEY_BYTES);
  const copy = raw.slice();
  let wrap: Uint8Array;
  try {
    wrap = await wrapSecret(
      keys.agreementPublic,
      { company, wrappedKind: "record", wrappedKeyId: keyId, recipientKind: "member", recipientKeyId: keys.keyId },
      copy,
    );
  } finally {
    wipe(copy);
  }
  const place = { company, record: id, revision: 1 };
  const body = await sealRevision(await importRecordKey(raw), { ...place, keyId }, encodeRecord(document));
  const signature = await signRevision(keys.signingPrivate, place, body);
  return {
    id,
    body: {
      id,
      record_type: document.type,
      key_id: hex(keyId),
      body: encode(body),
      signature: encode(signature),
      author_key: hex(keys.keyId),
      wrap: encode(wrap),
    },
  };
}
