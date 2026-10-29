// C7: an owner's browser recovers a member. It opens the company recovery key with its own key, opens the member's
// escrow with that, and wraps the member's keys to the one-time handoff key the member's browser made; the server
// carries wraps only, and a wrap made for one request or member opens for no other.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { handOver, type Recovery } from "../../src/crypto/recovery.ts";
import { unwrapSecret, wrapSecret, type WrapContext } from "../../src/crypto/wraps.ts";
import { noise, refusedWith, toHex } from "./support.ts";

const COMPANY = "company-1";

async function scene(): Promise<{ recovery: Recovery; memberKeys: Bytes; handoff: CryptoKeyPair; handoffPublic: Bytes }> {
  const owner = await generatePair("X25519");
  const ownerPublic = await publicBytes(owner.publicKey);
  const rescue = await generatePair("X25519");
  const rescuePublic = await publicBytes(rescue.publicKey);
  const rescueRaw = new Uint8Array(await crypto.subtle.exportKey("pkcs8", rescue.privateKey)).slice(-32);
  const ids = { recoveryKey: noise(16), ownerKey: noise(16), memberKey: noise(16), request: noise(16) };
  const toOwner: WrapContext = { company: COMPANY, wrappedKind: "recovery-to-owner", wrappedKeyId: ids.recoveryKey, recipientKind: "member", recipientKeyId: ids.ownerKey };
  const toRescue: WrapContext = { company: COMPANY, wrappedKind: "member-recovery", wrappedKeyId: ids.memberKey, recipientKind: "recovery", recipientKeyId: ids.recoveryKey };
  const memberKeys = noise(64);
  const handoff = await generatePair("X25519");
  const handoffPublic = await publicBytes(handoff.publicKey);
  return {
    recovery: {
      company: COMPANY,
      ownerPrivate: owner.privateKey,
      ownerPublic,
      ownerKeyId: ids.ownerKey,
      recoveryKeyId: ids.recoveryKey,
      recoveryPublic: rescuePublic,
      recoveryWrap: await wrapSecret(ownerPublic, toOwner, rescueRaw),
      memberKeyId: ids.memberKey,
      escrow: await wrapSecret(rescuePublic, toRescue, memberKeys),
      requestId: ids.request,
      handoffPublic,
    },
    memberKeys,
    handoff,
    handoffPublic,
  };
}

function handoffContext(recovery: Recovery, request: Bytes = recovery.requestId): WrapContext {
  return { company: COMPANY, wrappedKind: "member-recovery", wrappedKeyId: recovery.memberKeyId, recipientKind: "member", recipientKeyId: request };
}

describe("organisation recovery", () => {
  it("hands the member's own keys to the member's handoff key", async () => {
    const { recovery, memberKeys, handoff, handoffPublic } = await scene();
    const wrap = await handOver(recovery);
    const received = await unwrapSecret(handoff.privateKey, handoffPublic, handoffContext(recovery), wrap);
    assert.equal(toHex(received), toHex(memberKeys));
  });

  it("makes a handoff that opens for no other request", async () => {
    const { recovery, handoff, handoffPublic } = await scene();
    const wrap = await handOver(recovery);
    await assert.rejects(unwrapSecret(handoff.privateKey, handoffPublic, handoffContext(recovery, noise(16)), wrap));
  });

  it("refuses an escrow that is not this member's, or a recovery key wrap that is not this owner's", async () => {
    const { recovery } = await scene();
    await refusedWith(handOver({ ...recovery, memberKeyId: noise(16) }), "unauthentic");
    await refusedWith(handOver({ ...recovery, ownerKeyId: noise(16) }), "unauthentic");
  });
});
