// The keys a record wrap is opened with: the reader's own agreement key, or a group's key. A group's private key is
// kept on the server wrapped to each member (`GET /api/companies/{company}/groups/{group}/key`), and it is used only
// when the owner's chain vouches for the public half the server lists, so the server cannot put a key of its own
// between a group and its records.

import { sameBytes, wipe, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import type { OpenKeySet } from "../crypto/key-set.ts";
import { x25519PrivateFromBytes } from "../crypto/keys.ts";
import type { Vouched } from "../crypto/statements.ts";
import { unwrapSecret } from "../crypto/wraps.ts";
import type { RecipientKey } from "./open-record.ts";

/** A group's key as the server lists it: its id, public half, and the wrap kept for the caller, if any. */
export interface ListedGroupKey {
  readonly keyId: Bytes;
  readonly public: Bytes;
  readonly wrap: Bytes | null;
}

/** The member's own key, which opens wraps kept for `member:<their key id>`. */
export const memberKey = (keys: OpenKeySet): RecipientKey => ({
  kind: "member",
  keyId: keys.keyId,
  agreementPrivate: keys.agreementPrivate,
  agreementPublic: keys.agreementPublic,
});

/** The key of `group`, opened from the wrap kept for this member, refused unless the chain vouches for it. */
export async function groupKey(company: string, vouched: Vouched, member: OpenKeySet, group: string, listed: ListedGroupKey): Promise<RecipientKey> {
  const vouchedPublic = vouched.groupKeyOf(group);
  if (vouchedPublic === undefined || !sameBytes(vouchedPublic, listed.public)) {
    throw new VaultCryptoError("unvouched-recipient");
  }
  if (listed.wrap === null) {
    throw new VaultCryptoError("unknown-key");
  }
  const raw = await unwrapSecret(
    member.agreementPrivate,
    member.agreementPublic,
    { company, wrappedKind: "group", wrappedKeyId: listed.keyId, recipientKind: "member", recipientKeyId: member.keyId },
    listed.wrap,
  );
  try {
    return { kind: "group", keyId: listed.keyId, agreementPrivate: await x25519PrivateFromBytes(raw, false), agreementPublic: listed.public };
  } finally {
    wipe(raw);
  }
}
