// Opening a record of the opened page in the browser. The author's signature is checked first, against the signing key
// the owner's chain vouches for and over exactly this company, record and revision, so nothing the server could have
// written is decrypted or shown; then the record key comes out of the wrap kept for the reader's key, the body opens
// under it, and the plaintext is read as a record.

import { hex, wipe, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { importRecordKey, openRevision, verifyRevision } from "../crypto/record.ts";
import type { Vouched } from "../crypto/statements.ts";
import { unwrapSecret } from "../crypto/wraps.ts";
import type { OpenedRecord } from "../records-model.ts";
import { keyIdBytes } from "../unlock-model.ts";
import { decodeRecord, type RecordDocument } from "./document.ts";

/** The key a wrap is opened with: the reader's own agreement key, or a group key they hold. */
export interface RecipientKey {
  readonly kind: "member" | "group";
  readonly keyId: Bytes;
  readonly agreementPrivate: CryptoKey;
  readonly agreementPublic: Bytes;
}

/** Opens `opened` in `company` with `recipient`, trusting its author only as far as `vouched` does. */
export async function openRecord(company: string, vouched: Vouched, recipient: RecipientKey, opened: OpenedRecord): Promise<RecordDocument> {
  const { row } = opened;
  const keyId = keyIdBytes(row.keyId);
  if (keyId === null || opened.recipient.kind !== recipient.kind || opened.recipient.keyId !== hex(recipient.keyId)) {
    throw new VaultCryptoError("key-mismatch");
  }
  const place = { company, record: row.id, revision: row.revision };
  await verifyRevision(vouched, row.author, place, opened.body, opened.signature);
  const raw = await unwrapSecret(
    recipient.agreementPrivate,
    recipient.agreementPublic,
    { company, wrappedKind: "record", wrappedKeyId: keyId, recipientKind: recipient.kind, recipientKeyId: recipient.keyId },
    opened.wrap,
  );
  const recordKey = await importRecordKey(raw);
  const plaintext = await openRevision(new Map([[row.keyId, recordKey]]), { ...place, keyId }, opened.body);
  try {
    return decodeRecord(plaintext);
  } finally {
    wipe(plaintext);
  }
}
