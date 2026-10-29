// A record revision's body (algorithm 0x01): `[version][0x01][key id 16][salt 32][nonce 12][ciphertext ‖ tag]`.
// The record key (RK) only feeds HKDF; each revision gets its own content key CK = HKDF(RK, fresh salt,
// "tessaridb-vault/v1/record-content"), used for exactly one encryption, so a random 96-bit nonce never repeats under
// a key. The associated data is rebuilt from the caller's authoritative context (the request path and the metadata
// row), so a body copied to another record, revision or company does not open.

import { Reader, concat, field, hex, integer, random, sameBytes, utf8, wipe, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { ALGORITHM, FORMAT_VERSION, LABEL } from "./labels.ts";
import { sign, verify } from "./signatures.ts";
import type { Vouched } from "./statements.ts";

const KEY_ID_BYTES = 16;
const SALT_BYTES = 32;
const NONCE_BYTES = 12;
const TAG_BYTES = 16;

/** Where a revision belongs, from the server's rows and the request path, never from the body. */
export interface RecordContext {
  company: string;
  record: string;
  revision: number;
  keyId: Bytes;
}

/** Import a record key's 32 bytes as a non-extractable HKDF key, and wipe the bytes. */
export async function importRecordKey(raw: Bytes): Promise<CryptoKey> {
  if (raw.length !== 32) {
    throw new VaultCryptoError("malformed");
  }
  const key = await crypto.subtle.importKey("raw", raw, "HKDF", false, ["deriveKey"]);
  wipe(raw);
  return key;
}

function associatedData(context: RecordContext, salt: Bytes): Bytes {
  return concat(
    utf8(LABEL.record),
    field(Uint8Array.of(FORMAT_VERSION)),
    field(Uint8Array.of(ALGORITHM.recordBody)),
    field(utf8(context.company)),
    field(utf8(context.record)),
    field(integer(context.revision, 8)),
    field(context.keyId),
    field(salt),
  );
}

async function contentKey(recordKey: CryptoKey, salt: Bytes, usage: "encrypt" | "decrypt"): Promise<CryptoKey> {
  return crypto.subtle.deriveKey(
    { name: "HKDF", hash: "SHA-256", salt, info: utf8(LABEL.recordContent) },
    recordKey,
    { name: "AES-GCM", length: 256 },
    false,
    [usage],
  );
}

/** Seal one revision's plaintext under a fresh content key. */
export async function sealRevision(recordKey: CryptoKey, context: RecordContext, plaintext: Bytes): Promise<Bytes> {
  if (context.keyId.length !== KEY_ID_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  const salt = random(SALT_BYTES);
  const nonce = random(NONCE_BYTES);
  const key = await contentKey(recordKey, salt, "encrypt");
  const sealed = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce, additionalData: associatedData(context, salt) },
    key,
    plaintext,
  );
  return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.recordBody), context.keyId, salt, nonce, new Uint8Array(sealed));
}

/**
 * Open a revision with the record key the context names. A header naming another key is refused, and a key the
 * caller does not hold is refused without trying the keys it does hold.
 */
export async function openRevision(
  keys: ReadonlyMap<string, CryptoKey>,
  context: RecordContext,
  body: Bytes,
): Promise<Bytes> {
  const reader = new Reader(body);
  if (reader.integer(1) !== FORMAT_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  if (reader.integer(1) !== ALGORITHM.recordBody) {
    throw new VaultCryptoError("unknown-algorithm");
  }
  const keyId = reader.take(KEY_ID_BYTES);
  const salt = reader.take(SALT_BYTES);
  const nonce = reader.take(NONCE_BYTES);
  const ciphertext = reader.rest();
  if (ciphertext.length < TAG_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  if (!sameBytes(keyId, context.keyId)) {
    throw new VaultCryptoError("key-mismatch");
  }
  const recordKey = keys.get(hex(context.keyId));
  if (recordKey === undefined) {
    throw new VaultCryptoError("unknown-key");
  }
  const key = await contentKey(recordKey, salt, "decrypt");
  try {
    return new Uint8Array(
      await crypto.subtle.decrypt(
        { name: "AES-GCM", iv: nonce, additionalData: associatedData(context, salt) },
        key,
        ciphertext,
      ),
    );
  } catch {
    throw new VaultCryptoError("unauthentic");
  }
}

/** Where a revision belongs, for its author's signature: the same authoritative values as the body's context. */
export interface RevisionContext {
  company: string;
  record: string;
  revision: number;
  /** The seq of the last statement of the chain the author verified when signing (ADR: revisions bound to it). */
  chainSeq: number;
}

function signedRevision(context: RevisionContext, body: Bytes): Bytes {
  return concat(
    utf8(LABEL.revision),
    field(utf8(context.company)),
    field(utf8(context.record)),
    field(integer(context.revision, 8)),
    field(integer(context.chainSeq, 8)),
    field(body),
  );
}

/** The author's signature over a sealed revision (algorithm 0x04), with their unwrapped signing key. */
export async function signRevision(signingKey: CryptoKey, context: RevisionContext, body: Bytes): Promise<Bytes> {
  return sign(signingKey, signedRevision(context, body));
}

/**
 * Accept a revision only from an author the verified chain vouched for at the position the revision was signed at,
 * signed over exactly this place, position and body.
 */
export async function verifyRevision(
  vouched: Vouched,
  author: string,
  context: RevisionContext,
  body: Bytes,
  signature: Bytes,
): Promise<void> {
  const signingKey = vouched.signingKeyAt(author, context.chainSeq);
  if (signingKey === undefined) {
    throw new VaultCryptoError("unvouched-signer");
  }
  await verify(signingKey, signedRevision(context, body), signature);
}
