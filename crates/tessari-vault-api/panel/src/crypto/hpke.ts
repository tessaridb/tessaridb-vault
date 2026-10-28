// HPKE Base mode (RFC 9180) for one suite only: DHKEM(X25519, HKDF-SHA256) 0x0020, HKDF-SHA256 0x0001, AES-256-GCM
// 0x0002. It is the standard's key schedule written over WebCrypto, so every private key stays a non-extractable
// CryptoKey; it is single-shot (one encryption per context, sequence 0), which is all the vault ever needs.

import { concat, integer, utf8, wipe, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { expand, extract } from "./hkdf.ts";
import { generatePair, publicBytes, x25519, x25519PublicFromBytes } from "./keys.ts";

const KEM_ID = 0x0020;
const KDF_ID = 0x0001;
const AEAD_ID = 0x0002;
const MODE_BASE = 0x00;

const KEM_SUITE = concat(utf8("KEM"), integer(KEM_ID, 2));
const HPKE_SUITE = concat(utf8("HPKE"), integer(KEM_ID, 2), integer(KDF_ID, 2), integer(AEAD_ID, 2));
const VERSION_LABEL = utf8("HPKE-v1");

const SECRET_BYTES = 32;
const KEY_BYTES = 32;
const NONCE_BYTES = 12;
const HASH_BYTES = 32;

/** An ephemeral key pair; `seal` makes one per call, the test vectors supply theirs. */
export interface Ephemeral {
  privateKey: CryptoKey;
  publicBytes: Bytes;
}

async function labeledExtract(suite: Bytes, salt: Bytes, label: string, ikm: Bytes): Promise<Bytes> {
  return extract(salt, concat(VERSION_LABEL, suite, utf8(label), ikm));
}

async function labeledExpand(suite: Bytes, prk: Bytes, label: string, info: Bytes, length: number): Promise<Bytes> {
  return expand(prk, concat(integer(length, 2), VERSION_LABEL, suite, utf8(label), info), length);
}

/** DHKEM ExtractAndExpand (RFC 9180 §4.1). */
async function sharedSecretOf(dh: Bytes, enc: Bytes, recipientPublic: Bytes): Promise<Bytes> {
  const prk = await labeledExtract(KEM_SUITE, new Uint8Array(0), "eae_prk", dh);
  const secret = await labeledExpand(KEM_SUITE, prk, "shared_secret", concat(enc, recipientPublic), SECRET_BYTES);
  wipe(prk, dh);
  return secret;
}

/** Encap with a given ephemeral pair; the encapsulated key is the ephemeral public key. */
export async function encapsulateWith(
  ephemeral: Ephemeral,
  recipientPublic: Bytes,
): Promise<{ sharedSecret: Bytes; enc: Bytes }> {
  const dh = await x25519(ephemeral.privateKey, await x25519PublicFromBytes(recipientPublic));
  const enc = ephemeral.publicBytes.slice();
  return { sharedSecret: await sharedSecretOf(dh, enc, recipientPublic), enc };
}

/** KeyScheduleS for mode Base with no PSK (RFC 9180 §5.1). */
export async function keySchedule(
  sharedSecret: Bytes,
  info: Bytes,
): Promise<{ key: Bytes; baseNonce: Bytes; exporterSecret: Bytes }> {
  const empty = new Uint8Array(0);
  const pskIdHash = await labeledExtract(HPKE_SUITE, empty, "psk_id_hash", empty);
  const infoHash = await labeledExtract(HPKE_SUITE, empty, "info_hash", info);
  const context = concat(Uint8Array.of(MODE_BASE), pskIdHash, infoHash);
  const secret = await labeledExtract(HPKE_SUITE, sharedSecret, "secret", empty);
  const key = await labeledExpand(HPKE_SUITE, secret, "key", context, KEY_BYTES);
  const baseNonce = await labeledExpand(HPKE_SUITE, secret, "base_nonce", context, NONCE_BYTES);
  const exporterSecret = await labeledExpand(HPKE_SUITE, secret, "exp", context, HASH_BYTES);
  wipe(secret);
  return { key, baseNonce, exporterSecret };
}

async function aeadKey(sharedSecret: Bytes, info: Bytes): Promise<{ key: CryptoKey; nonce: Bytes }> {
  const schedule = await keySchedule(sharedSecret, info);
  const key = await crypto.subtle.importKey("raw", schedule.key, { name: "AES-GCM" }, false, ["encrypt", "decrypt"]);
  wipe(schedule.key, schedule.exporterSecret, sharedSecret);
  return { key, nonce: schedule.baseNonce };
}

/** SealBase with a given ephemeral pair (the vectors' seam); `seal` is the one the vault calls. */
export async function sealWith(
  ephemeral: Ephemeral,
  recipientPublic: Bytes,
  info: Bytes,
  aad: Bytes,
  plaintext: Bytes,
): Promise<{ enc: Bytes; ciphertext: Bytes }> {
  const { sharedSecret, enc } = await encapsulateWith(ephemeral, recipientPublic);
  const { key, nonce } = await aeadKey(sharedSecret, info);
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce, additionalData: aad }, key, plaintext);
  return { enc, ciphertext: new Uint8Array(ciphertext) };
}

/** SealBase with a fresh ephemeral pair. */
export async function seal(
  recipientPublic: Bytes,
  info: Bytes,
  aad: Bytes,
  plaintext: Bytes,
): Promise<{ enc: Bytes; ciphertext: Bytes }> {
  const pair = await generatePair("X25519", false);
  return sealWith({ privateKey: pair.privateKey, publicBytes: await publicBytes(pair.publicKey) }, recipientPublic, info, aad, plaintext);
}

/** OpenBase; every failure after the key agreement is `unauthentic`, whichever input was wrong. */
export async function open(
  recipientPrivate: CryptoKey,
  recipientPublic: Bytes,
  enc: Bytes,
  info: Bytes,
  aad: Bytes,
  ciphertext: Bytes,
): Promise<Bytes> {
  if (enc.length !== SECRET_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  let dh: Bytes;
  try {
    dh = await x25519(recipientPrivate, await x25519PublicFromBytes(enc));
  } catch {
    throw new VaultCryptoError("unauthentic");
  }
  const { key, nonce } = await aeadKey(await sharedSecretOf(dh, enc, recipientPublic), info);
  try {
    return new Uint8Array(await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce, additionalData: aad }, key, ciphertext));
  } catch {
    throw new VaultCryptoError("unauthentic");
  }
}
