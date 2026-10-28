// A key wrapped to one recipient (algorithm 0x02): `[version][0x02][enc 32][ciphertext ‖ tag]`, an HPKE Base seal
// whose `info` binds the company, what is wrapped and to whom. The context comes from the rows the server returned,
// never from the wrap, so a wrap moved to another record, recipient or company does not open.

import { Reader, concat, field, utf8, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { open, seal } from "./hpke.ts";
import { ALGORITHM, FORMAT_VERSION, LABEL } from "./labels.ts";

const ENC_BYTES = 32;
const KEY_ID_BYTES = 16;

export type WrappedKind = "record" | "group" | "member-recovery" | "recovery-to-owner";
export type RecipientKind = "member" | "group" | "recovery";

/** What a wrap is for, from the server's authoritative rows. */
export interface WrapContext {
  company: string;
  wrappedKind: WrappedKind;
  wrappedKeyId: Bytes;
  recipientKind: RecipientKind;
  recipientKeyId: Bytes;
}

function infoOf(context: WrapContext): Bytes {
  if (context.wrappedKeyId.length !== KEY_ID_BYTES || context.recipientKeyId.length !== KEY_ID_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  return concat(
    utf8(LABEL.wrap),
    field(utf8(context.company)),
    field(utf8(context.wrappedKind)),
    field(context.wrappedKeyId),
    field(utf8(context.recipientKind)),
    field(context.recipientKeyId),
  );
}

/** Wrap `secret` (a record, group or member key) to the recipient's X25519 public key. */
export async function wrapSecret(recipientPublic: Bytes, context: WrapContext, secret: Bytes): Promise<Bytes> {
  const sealed = await seal(recipientPublic, infoOf(context), new Uint8Array(0), secret);
  return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.keyWrap), sealed.enc, sealed.ciphertext);
}

/** Open a wrap with the recipient's private key; the caller wipes the secret once it is imported. */
export async function unwrapSecret(
  recipientPrivate: CryptoKey,
  recipientPublic: Bytes,
  context: WrapContext,
  wrap: Bytes,
): Promise<Bytes> {
  const reader = new Reader(wrap);
  if (reader.integer(1) !== FORMAT_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  if (reader.integer(1) !== ALGORITHM.keyWrap) {
    throw new VaultCryptoError("unknown-algorithm");
  }
  const enc = reader.take(ENC_BYTES);
  const ciphertext = reader.rest();
  return open(recipientPrivate, recipientPublic, enc, infoOf(context), new Uint8Array(0), ciphertext);
}
