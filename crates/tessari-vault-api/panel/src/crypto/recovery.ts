// C7, in an owner's browser: the company recovery key opens with the owner's own key, the member's escrow opens with
// the recovery key, and the member's keys are wrapped to the one-time handoff key the member's browser made for this
// request. Each wrap's context comes from the server's rows, so a wrap made for one member, owner or request opens for
// no other; the recovery key and the member's keys are wiped here once the handoff is sealed.

import { wipe, type Bytes } from "./bytes.ts";
import { x25519PrivateFromBytes } from "./keys.ts";
import { unwrapSecret, wrapSecret } from "./wraps.ts";

/** What an owner's browser holds and reads to recover one member. */
export interface Recovery {
  company: string;
  /** The owner's own X25519 key, unlocked. */
  ownerPrivate: CryptoKey;
  ownerPublic: Bytes;
  ownerKeyId: Bytes;
  recoveryKeyId: Bytes;
  /** The recovery key's public half, as the owner's chain vouched for it. */
  recoveryPublic: Bytes;
  /** The owner's wrap of the recovery key. */
  recoveryWrap: Bytes;
  memberKeyId: Bytes;
  /** The member's keys wrapped to the recovery key. */
  escrow: Bytes;
  requestId: Bytes;
  /** The member's one-time handoff key for this request. */
  handoffPublic: Bytes;
}

/** Wrap the member's escrowed keys to their handoff key for this request. */
export async function handOver(recovery: Recovery): Promise<Bytes> {
  const { company } = recovery;
  const rescue = await unwrapSecret(
    recovery.ownerPrivate,
    recovery.ownerPublic,
    {
      company,
      wrappedKind: "recovery-to-owner",
      wrappedKeyId: recovery.recoveryKeyId,
      recipientKind: "member",
      recipientKeyId: recovery.ownerKeyId,
    },
    recovery.recoveryWrap,
  );
  let memberKeys: Bytes | undefined;
  try {
    const rescuePrivate = await x25519PrivateFromBytes(rescue, false);
    memberKeys = await unwrapSecret(
      rescuePrivate,
      recovery.recoveryPublic,
      {
        company,
        wrappedKind: "member-recovery",
        wrappedKeyId: recovery.memberKeyId,
        recipientKind: "recovery",
        recipientKeyId: recovery.recoveryKeyId,
      },
      recovery.escrow,
    );
    return await wrapSecret(
      recovery.handoffPublic,
      {
        company,
        wrappedKind: "member-recovery",
        wrappedKeyId: recovery.memberKeyId,
        recipientKind: "member",
        recipientKeyId: recovery.requestId,
      },
      memberKeys,
    );
  } finally {
    wipe(rescue);
    if (memberKeys !== undefined) {
      wipe(memberKeys);
    }
  }
}
