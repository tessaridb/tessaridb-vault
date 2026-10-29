// The escrow plaintext (C7): what a member's browser wraps to the company recovery key and gets back after a recovery,
// `[1][field agreement pkcs8][field signing pkcs8][owner signing public 32]`. The owner key rides along because a
// member who lost the unlock password cannot open their old pin, and the restored key set must pin the same owner key.
// The bytes exist only between their export or unwrap and their wrap or import, and the caller wipes them.

import { Reader, concat, field, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";

const ESCROW_VERSION = 1;
const OWNER_KEY_BYTES = 32;

/** A member's private keys as PKCS #8, with the owner key their key set pins. */
export interface Escrow {
  readonly agreement: Bytes;
  readonly signing: Bytes;
  readonly ownerSigningPublic: Bytes;
}

export const escrowBytes = (escrow: Escrow): Bytes =>
  concat(Uint8Array.of(ESCROW_VERSION), field(escrow.agreement), field(escrow.signing), escrow.ownerSigningPublic);

/** The escrow in `bytes`; `unknown-version` for another layout, `malformed` for a damaged one. */
export function readEscrow(bytes: Bytes): Escrow {
  const reader = new Reader(bytes);
  if (reader.integer(1) !== ESCROW_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  const agreement = reader.field();
  const signing = reader.field();
  const ownerSigningPublic = reader.take(OWNER_KEY_BYTES);
  reader.end();
  return { agreement, signing, ownerSigningPublic };
}
