// HKDF-SHA256 (RFC 5869) as its two halves over WebCrypto HMAC. HPKE's key schedule needs Extract and Expand
// separately; the record and unlock keys use WebCrypto's own HKDF instead and never pass through here.

import { concat, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";

const HASH_BYTES = 32;

async function hmac(key: Bytes, message: Bytes): Promise<Bytes> {
  const imported = await crypto.subtle.importKey("raw", key, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  return new Uint8Array(await crypto.subtle.sign("HMAC", imported, message));
}

/** HKDF-Extract: an empty salt is a string of zero bytes the length of the hash (RFC 5869 §2.2). */
export async function extract(salt: Bytes, ikm: Bytes): Promise<Bytes> {
  return hmac(salt.length === 0 ? new Uint8Array(HASH_BYTES) : salt, ikm);
}

/** HKDF-Expand to `length` bytes, at most 255 blocks of the hash. */
export async function expand(prk: Bytes, info: Bytes, length: number): Promise<Bytes> {
  if (!Number.isInteger(length) || length < 1 || length > 255 * HASH_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  const blocks: Bytes[] = [];
  let previous: Bytes = new Uint8Array(0);
  for (let counter = 1; blocks.length * HASH_BYTES < length; counter += 1) {
    previous = await hmac(prk, concat(previous, info, Uint8Array.of(counter)));
    blocks.push(previous);
  }
  return concat(...blocks).slice(0, length);
}
