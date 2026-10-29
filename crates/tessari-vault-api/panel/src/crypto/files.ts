// Files (algorithm 0x05): `[version][0x05][key id 16][salt 32][prefix 7]` then 64 KiB chunks, each AES-256-GCM under
// HKDF(FK, salt, "tessaridb-vault/v1/file") with nonce = prefix ‖ u32 chunk number ‖ u8 last-chunk flag (the STREAM
// construction). A chunk moved, dropped or added changes a nonce and fails its tag; a file cut at a chunk boundary
// fails because its new last chunk was sealed as not-last. FK is the file's own random key, at most 4 096 chunks under
// it; the file field keeps FK and its id inside the record's sealed body, so each new revision, sealed under a new
// record key, carries the same FK to every reader and the file itself is never sealed again.

import { Reader, concat, field, hex, integer, random, sameBytes, utf8, wipe, type Bytes } from "./bytes.ts";
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

const FILE_KEY_BYTES = 32;

const toHex = (bytes: Uint8Array): string => Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");

function fromHex(text: string, length: number): Bytes {
  if (!new RegExp(`^[0-9a-f]{${2 * length}}$`).test(text)) {
    throw new VaultCryptoError("malformed");
  }
  return Uint8Array.from(text.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

/** A new file's own key and the id its header names, in the lower-case hex a file field keeps them in. */
export function newFileKey(): { readonly key: string; readonly keyId: string } {
  const key = random(FILE_KEY_BYTES);
  const made = { key: toHex(key), keyId: toHex(random(KEY_ID_BYTES)) };
  wipe(key);
  return made;
}

/** A file field's key, ready to seal or open its file; the decoded bytes are wiped once imported. */
export async function importFileKey(key: string): Promise<CryptoKey> {
  const raw = fromHex(key, FILE_KEY_BYTES);
  const imported = await crypto.subtle.importKey("raw", raw, "HKDF", false, ["deriveKey"]);
  wipe(raw);
  return imported;
}

/** A file field's key id, as the file's context and header carry it. */
export const fileKeyId = (keyId: string): Bytes => fromHex(keyId, KEY_ID_BYTES);

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

async function chunkKey(fileKey: CryptoKey, salt: Bytes, usage: "encrypt" | "decrypt"): Promise<CryptoKey> {
  return crypto.subtle.deriveKey(
    { name: "HKDF", hash: "SHA-256", salt, info: utf8(LABEL.file) },
    fileKey,
    { name: "AES-GCM", length: 256 },
    false,
    [usage],
  );
}

/** Seal a whole file; refused above the ceiling before any byte of it is read. */
export async function sealFile(fileKey: CryptoKey, context: FileContext, data: Bytes): Promise<Bytes> {
  if (data.length > FILE_MAX_BYTES) {
    throw new VaultCryptoError("too-large");
  }
  if (context.keyId.length !== KEY_ID_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  const salt = random(SALT_BYTES);
  const prefix = random(PREFIX_BYTES);
  const key = await chunkKey(fileKey, salt, "encrypt");
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

/** Open a whole file with the file key the context names; any tampered, missing or extra chunk refuses it. */
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
  const fileKey = keys.get(hex(context.keyId));
  if (fileKey === undefined) {
    throw new VaultCryptoError("unknown-key");
  }
  const key = await chunkKey(fileKey, salt, "decrypt");
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
