// X25519 and Ed25519 keys as WebCrypto keys. A private key is imported or generated extractable only when it is about
// to be wrapped; everywhere else it is a non-extractable CryptoKey, so its bytes are out of the page's reach.

import { concat, sameBytes, wipe, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";

/** The PKCS #8 envelope of a 32-byte X25519 or Ed25519 private key (RFC 8410 §7): only the OID byte differs. */
const PKCS8_PREFIX_X25519 = Uint8Array.of(0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22, 0x04, 0x20);
const PKCS8_PREFIX_ED25519 = Uint8Array.of(0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20);

const KEY_BYTES = 32;

function exactKey(raw: Bytes): Bytes {
  if (raw.length !== KEY_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  return raw;
}

export async function x25519PrivateFromBytes(raw: Bytes, extractable: boolean): Promise<CryptoKey> {
  const pkcs8 = concat(PKCS8_PREFIX_X25519, exactKey(raw));
  try {
    return await crypto.subtle.importKey("pkcs8", pkcs8, { name: "X25519" }, extractable, ["deriveBits"]);
  } finally {
    wipe(pkcs8);
  }
}

export async function x25519PublicFromBytes(raw: Bytes): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", exactKey(raw), { name: "X25519" }, true, []);
}

export async function ed25519PrivateFromSeed(seed: Bytes, extractable: boolean): Promise<CryptoKey> {
  const pkcs8 = concat(PKCS8_PREFIX_ED25519, exactKey(seed));
  try {
    return await crypto.subtle.importKey("pkcs8", pkcs8, { name: "Ed25519" }, extractable, ["sign"]);
  } finally {
    wipe(pkcs8);
  }
}

export async function ed25519PublicFromBytes(raw: Bytes): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", exactKey(raw), { name: "Ed25519" }, true, ["verify"]);
}

/** The raw 32 bytes of a public key. */
export async function publicBytes(publicKey: CryptoKey): Promise<Bytes> {
  return new Uint8Array(await crypto.subtle.exportKey("raw", publicKey));
}

/**
 * A fresh pair. `extractable` is true only for a private half about to be wrapped (a member's or a group's key); an
 * HPKE ephemeral key is generated non-extractable.
 */
export async function generatePair(algorithm: "X25519" | "Ed25519", extractable = true): Promise<CryptoKeyPair> {
  const usages: KeyUsage[] = algorithm === "X25519" ? ["deriveBits"] : ["sign", "verify"];
  const generated = await crypto.subtle.generateKey({ name: algorithm }, extractable, usages);
  if (!("privateKey" in generated)) {
    throw new VaultCryptoError("malformed");
  }
  return generated;
}

/**
 * The X25519 shared secret. An all-zero result means the peer sent a small-order point (RFC 7748 §6.1, RFC 9180
 * §7.1.4) and is refused, whether the platform throws for it or returns the zeros.
 */
export async function x25519(privateKey: CryptoKey, publicKey: CryptoKey): Promise<Bytes> {
  let shared: Bytes;
  try {
    shared = new Uint8Array(await crypto.subtle.deriveBits({ name: "X25519", public: publicKey }, privateKey, 256));
  } catch {
    throw new VaultCryptoError("weak-shared-secret");
  }
  if (sameBytes(shared, new Uint8Array(KEY_BYTES))) {
    throw new VaultCryptoError("weak-shared-secret");
  }
  return shared;
}
