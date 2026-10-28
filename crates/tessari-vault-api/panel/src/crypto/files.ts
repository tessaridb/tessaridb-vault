// Files (algorithm 0x05): `[version][0x05][key id 16][salt 32][prefix 7]` then 64 KiB chunks, each AES-256-GCM under
// FK = HKDF(RK, salt, "tessaridb-vault/v1/file") with nonce = prefix ‖ u32 chunk number ‖ u8 last-chunk flag (the
// STREAM construction). A chunk moved, dropped or added changes a nonce and fails its tag; a file cut at a chunk
// boundary fails because its new last chunk was sealed as not-last. One FK per file, at most 4 096 chunks.

import { Reader, concat, field, hex, integer, random, sameBytes, utf8, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { ALGORITHM, FORMAT_VERSION, LABEL } from "./labels.ts";

export const FILE_CHUNK_BYTES = 64 * 1024;
/** The ceiling per file in v1: the browser holds a whole file in memory while it seals or opens it. */
export const FILE_MAX_BYTES = 256 * 1024 * 1024;

const KEY_ID_BYTES = 16;
const SALT_BYTES = 32;
const PREFIX_BYTES = 7;
const TAG_BYTES = 16;
const SEALED_CHUNK_BYTES = FILE_CHUNK_BYTES + TAG_BYTES;
const MAX_CHUNKS = FILE_MAX_BYTES / FILE_CHUNK_BYTES;

/** Which file this is, from the server's rows: never from the file's own header. */
export interface FileContext {
  company: string;
  record: string;
  file: string;
  keyId: Bytes;
}

function associatedData(context: FileContext, salt: Bytes, prefix: Bytes): Bytes {
  return concat(
    utf8(LABEL.fileChunk),
    field(utf8(context.company)),
    field(utf8(context.record)),
    field(utf8(context.file)),
    field(context.keyId),
    field(salt),
    field(prefix),
  );
}

function nonce(prefix: Bytes, chunk: number, last: boolean): Bytes {
  return concat(prefix, integer(chunk, 4), Uint8Array.of(last ? 1 : 0));
}

async function fileKey(recordKey: CryptoKey, salt: Bytes, usage: "encrypt" | "decrypt"): Promise<CryptoKey> {
  return crypto.subtle.deriveKey(
    { name: "HKDF", hash: "SHA-256", salt, info: utf8(LABEL.file) },
    recordKey,
    { name: "AES-GCM", length: 256 },
    false,
    [usage],
  );
}

/** Seal a whole file; refused above the ceiling before any byte of it is read. */
export async function sealFile(recordKey: CryptoKey, context: FileContext, data: Bytes): Promise<Bytes> {
  if (data.length > FILE_MAX_BYTES) {
    throw new VaultCryptoError("too-large");
  }
  if (context.keyId.length !== KEY_ID_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  const salt = random(SALT_BYTES);
  const prefix = random(PREFIX_BYTES);
  const key = await fileKey(recordKey, salt, "encrypt");
  const additionalData = associatedData(context, salt, prefix);
  const count = Math.max(1, Math.ceil(data.length / FILE_CHUNK_BYTES));
  const sealed: Bytes[] = [Uint8Array.of(FORMAT_VERSION, ALGORITHM.file), context.keyId, salt, prefix];
  for (let chunk = 0; chunk < count; chunk += 1) {
    const plaintext = data.subarray(chunk * FILE_CHUNK_BYTES, (chunk + 1) * FILE_CHUNK_BYTES);
    const iv = nonce(prefix, chunk, chunk === count - 1);
    sealed.push(new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv, additionalData }, key, plaintext)));
  }
  return concat(...sealed);
}

/** Open a whole file with the record key the context names; any tampered, missing or extra chunk refuses it. */
export async function openFile(keys: ReadonlyMap<string, CryptoKey>, context: FileContext, sealed: Bytes): Promise<Bytes> {
  const reader = new Reader(sealed);
  if (reader.integer(1) !== FORMAT_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  if (reader.integer(1) !== ALGORITHM.file) {
    throw new VaultCryptoError("unknown-algorithm");
  }
  const keyId = reader.take(KEY_ID_BYTES);
  const salt = reader.take(SALT_BYTES);
  const prefix = reader.take(PREFIX_BYTES);
  const body = reader.rest();
  const count = Math.max(1, Math.ceil(body.length / SEALED_CHUNK_BYTES));
  const lastLength = body.length - (count - 1) * SEALED_CHUNK_BYTES;
  if (lastLength < TAG_BYTES || count > MAX_CHUNKS) {
    throw new VaultCryptoError("malformed");
  }
  if (!sameBytes(keyId, context.keyId)) {
    throw new VaultCryptoError("key-mismatch");
  }
  const recordKey = keys.get(hex(context.keyId));
  if (recordKey === undefined) {
    throw new VaultCryptoError("unknown-key");
  }
  const key = await fileKey(recordKey, salt, "decrypt");
  const additionalData = associatedData(context, salt, prefix);
  const plain: Bytes[] = [];
  for (let chunk = 0; chunk < count; chunk += 1) {
    const ciphertext = body.subarray(chunk * SEALED_CHUNK_BYTES, (chunk + 1) * SEALED_CHUNK_BYTES);
    const iv = nonce(prefix, chunk, chunk === count - 1);
    try {
      plain.push(new Uint8Array(await crypto.subtle.decrypt({ name: "AES-GCM", iv, additionalData }, key, ciphertext)));
    } catch {
      throw new VaultCryptoError("unauthentic");
    }
  }
  return concat(...plain);
}
