// An Ed25519 signature as the vault stores it (algorithm 0x04): `[version][0x04][signature 64]`, so a later signature
// scheme can sit beside it. Every signed message starts with its own domain label, so a statement signature can never
// be replayed as a revision signature or the other way round.

import { Reader, concat, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { ed25519PublicFromBytes } from "./keys.ts";
import { ALGORITHM, FORMAT_VERSION } from "./labels.ts";

const SIGNATURE_BYTES = 64;

export async function sign(privateKey: CryptoKey, message: Bytes): Promise<Bytes> {
  const signature = new Uint8Array(await crypto.subtle.sign("Ed25519", privateKey, message));
  return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.signature), signature);
}

/** Verify a stored signature by the 32-byte Ed25519 public key; any failure to verify is `bad-signature`. */
export async function verify(publicKey: Bytes, message: Bytes, stored: Bytes): Promise<void> {
  const reader = new Reader(stored);
  if (reader.integer(1) !== FORMAT_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  if (reader.integer(1) !== ALGORITHM.signature) {
    throw new VaultCryptoError("unknown-algorithm");
  }
  const signature = reader.take(SIGNATURE_BYTES);
  reader.end();
  let valid: boolean;
  try {
    valid = await crypto.subtle.verify("Ed25519", await ed25519PublicFromBytes(publicKey), signature, message);
  } catch {
    valid = false;
  }
  if (!valid) {
    throw new VaultCryptoError("bad-signature");
  }
}
