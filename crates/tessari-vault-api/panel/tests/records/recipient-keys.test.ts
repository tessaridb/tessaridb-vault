// The keys a record wrap is opened with: the reader's own agreement key, or a group's key that the server keeps wrapped
// to the reader and the owner's chain vouches for.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { random, type Bytes } from "../../src/crypto/bytes.ts";
import type { OpenKeySet } from "../../src/crypto/key-set.ts";
import { generatePair, publicBytes, x25519, x25519PublicFromBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Statement } from "../../src/crypto/statements.ts";
import { wrapSecret } from "../../src/crypto/wraps.ts";
import { groupKey, memberKey } from "../../src/records/recipient-keys.ts";
import { refusedWith } from "../crypto/support.ts";

const COMPANY = "company-1";

/** A member's opened keys, a group "team-1" whose key the chain vouches for, and that key wrapped to the member. */
async function world() {
  const ownerSigning = await generatePair("Ed25519");
  const signer = { privateKey: ownerSigning.privateKey, publicBytes: await publicBytes(ownerSigning.publicKey) };
  const agreement = await generatePair("X25519");
  const agreementPublic = await publicBytes(agreement.publicKey);
  const member: OpenKeySet = {
    keyId: random(16),
    agreementPrivate: agreement.privateKey,
    signingPrivate: ownerSigning.privateKey,
    agreementPublic,
    signingPublic: signer.publicBytes,
    ownerSigningPublic: signer.publicBytes,
  };
  const team = await generatePair("X25519");
  const teamPublic = await publicBytes(team.publicKey);
  const teamPrivate = new Uint8Array(await crypto.subtle.exportKey("pkcs8", team.privateKey)).slice(-32);
  const statements: Statement[] = [];
  for (const fields of [
    { kind: "member" as const, subject: "owner", agreementPublic, signingPublic: signer.publicBytes },
    { kind: "group" as const, subject: "team-1", agreementPublic: teamPublic, signingPublic: new Uint8Array(0) },
  ]) {
    statements.push(await appendStatement(COMPANY, statements.at(-1), fields, signer));
  }
  const vouched = await verifyChain(COMPANY, signer.publicBytes, statements);
  const keyId = random(16);
  const wrapFor = (secret: Bytes, wrappedKeyId = keyId): Promise<Bytes> =>
    wrapSecret(agreementPublic, { company: COMPANY, wrappedKind: "group", wrappedKeyId, recipientKind: "member", recipientKeyId: member.keyId }, secret);
  return { member, vouched, team, teamPublic, teamPrivate, keyId, wrapFor };
}

/** Whether `opened` agrees with the group's public key: the same shared secret as the group's own private key. */
async function agreesWith(opened: CryptoKey, team: CryptoKeyPair): Promise<boolean> {
  const peer = await generatePair("X25519");
  const peerPublic = await x25519PublicFromBytes(await publicBytes(peer.publicKey));
  return Buffer.from(await x25519(opened, peerPublic)).equals(Buffer.from(await x25519(team.privateKey, peerPublic)));
}

describe("the keys a record wrap opens with", () => {
  it("is the member's own agreement key under their key id", async () => {
    const { member } = await world();
    const key = memberKey(member);
    assert.equal(key.kind, "member");
    assert.deepEqual(key.keyId, member.keyId);
    assert.equal(key.agreementPrivate, member.agreementPrivate);
    assert.deepEqual(key.agreementPublic, member.agreementPublic);
  });

  it("opens a group key wrapped to the member into a key that agrees with the vouched group key", async () => {
    const { member, vouched, team, teamPublic, teamPrivate, keyId, wrapFor } = await world();
    const key = await groupKey(COMPANY, vouched, member, "team-1", { keyId, public: teamPublic, wrap: await wrapFor(teamPrivate.slice()) });
    assert.equal(key.kind, "group");
    assert.deepEqual(key.keyId, keyId);
    assert.deepEqual(key.agreementPublic, teamPublic);
    assert.equal(key.agreementPrivate.extractable, false);
    assert.equal(await agreesWith(key.agreementPrivate, team), true);
  });

  it("refuses a group key the chain does not vouch for, one listed for another group, or one with no wrap", async () => {
    const { member, vouched, teamPublic, teamPrivate, keyId, wrapFor } = await world();
    const stranger = await publicBytes((await generatePair("X25519")).publicKey);
    const wrap = await wrapFor(teamPrivate.slice());
    await refusedWith(groupKey(COMPANY, vouched, member, "team-1", { keyId, public: stranger, wrap }), "unvouched-recipient");
    await refusedWith(groupKey(COMPANY, vouched, member, "team-2", { keyId, public: teamPublic, wrap }), "unvouched-recipient");
    await refusedWith(groupKey(COMPANY, vouched, member, "team-1", { keyId, public: teamPublic, wrap: null }), "unknown-key");
  });

  it("refuses a wrap made for another group key id", async () => {
    const { member, vouched, teamPublic, teamPrivate, keyId, wrapFor } = await world();
    const wrap = await wrapFor(teamPrivate.slice(), random(16));
    await refusedWith(groupKey(COMPANY, vouched, member, "team-1", { keyId, public: teamPublic, wrap }), "unauthentic");
  });
});
