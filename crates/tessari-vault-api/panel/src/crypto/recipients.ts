// NT-33: a key is wrapped only to a key the owner's chain vouches for. The server lists members' and groups' public
// keys; the browser takes a listed key only when it is the one the verified chain names for that member or group, so
// a server that writes a key of its own into the directory gains no reader.

import { sameBytes, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import type { Vouched } from "./statements.ts";
import { wrapSecret, type WrapContext } from "./wraps.ts";

/** Whom a key is wrapped to. */
export type Recipient = { kind: "member"; account: string } | { kind: "group"; group: string } | { kind: "recovery" };

function vouchedKey(chain: Vouched, recipient: Recipient): Bytes | undefined {
  switch (recipient.kind) {
    case "member":
      return chain.agreementKeyOf(recipient.account);
    case "group":
      return chain.groupKeyOf(recipient.group);
    case "recovery":
      return chain.recoveryKey()?.agreementPublic;
  }
}

/** Wrap `secret` to `recipient`, whose key the server listed as `listed`, refused unless the chain vouches for it. */
export async function wrapToVouched(
  chain: Vouched,
  recipient: Recipient,
  listed: Bytes,
  context: WrapContext,
  secret: Bytes,
): Promise<Bytes> {
  if (context.recipientKind !== recipient.kind) {
    throw new VaultCryptoError("malformed");
  }
  const vouched = vouchedKey(chain, recipient);
  if (vouched === undefined || !sameBytes(vouched, listed)) {
    throw new VaultCryptoError("unvouched-recipient");
  }
  return wrapSecret(vouched, context, secret);
}
