// The company recovery key, made in the owner's browser: vouched in the chain by the owner key alone, its private half
// wrapped to every owner the chain vouches for; and the handoff code a member and an owner compare before a hand-over.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { hex, type Bytes } from "../../src/crypto/bytes.ts";
import { handoffCode } from "../../src/crypto/fingerprint.ts";
import { generatePair, publicBytes, x25519PrivateFromBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Signer, type Statement } from "../../src/crypto/statements.ts";
import { unwrapSecret } from "../../src/crypto/wraps.ts";
import { newRecoveryKey, type OwnerKey } from "../../src/recovery/recovery-key.ts";

const COMPANY = "company-1";

interface Person {
  readonly account: string;
  readonly signer: Signer;
  readonly agreement: CryptoKeyPair;
  readonly agreementPublic: Bytes;
  readonly keyId: Bytes;
}

async function person(account: string): Promise<Person> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  const keyId = crypto.getRandomValues(new Uint8Array(16));
  return { account, signer: { privateKey: signing.privateKey, publicBytes: await publicBytes(signing.publicKey) }, agreement, agreementPublic: await publicBytes(agreement.publicKey), keyId };
}

const member = (who: Person) => ({ kind: "member" as const, subject: who.account, agreementPublic: who.agreementPublic, signingPublic: who.signer.publicBytes });

/** A chain begun by `owner`, vouching `others` as members and making `admin` an admin. */
async function chainOf(owner: Person, others: readonly Person[], admin?: Person): Promise<Statement[]> {
  const chain = [await appendStatement(COMPANY, undefined, member(owner), owner.signer)];
  for (const other of others) {
    chain.push(await appendStatement(COMPANY, chain.at(-1), member(other), owner.signer));
  }
  if (admin !== undefined) {
    chain.push(await appendStatement(COMPANY, chain.at(-1), { kind: "admin", subject: admin.account, agreementPublic: new Uint8Array(0), signingPublic: new Uint8Array(0) }, owner.signer));
  }
  return chain;
}

const ownerKey = (who: Person): OwnerKey => ({ account: who.account, keyId: who.keyId, agreementPublic: who.agreementPublic });

describe("the company recovery key", () => {
  it("is vouched by the owner key and opens from each vouched owner's own wrap, and from no other", async () => {
    const [founder, second, stranger] = await Promise.all([person("acc-1"), person("acc-2"), person("acc-3")]);
    const chain = await chainOf(founder, [second]);
    const vouched = await verifyChain(COMPANY, founder.signer.publicBytes, chain);
    const made = await newRecoveryKey(COMPANY, founder.signer, founder.signer.publicBytes, chain, vouched, [ownerKey(founder), ownerKey(second), ownerKey(stranger)]);
    const standing = (await verifyChain(COMPANY, founder.signer.publicBytes, [...chain, made.statement])).recoveryKey();
    assert.equal(standing?.keyId, hex(made.keyId));
    assert.deepEqual(standing?.agreementPublic, made.public);
    assert.deepEqual(made.unvouched, ["acc-3"]);
    assert.deepEqual(made.wraps.map((each) => hex(each.keyId)), [hex(founder.keyId), hex(second.keyId)]);
    for (const [at, owner] of [founder, second].entries()) {
      const context = { company: COMPANY, wrappedKind: "recovery-to-owner", wrappedKeyId: made.keyId, recipientKind: "member", recipientKeyId: owner.keyId } as const;
      const raw = await unwrapSecret(owner.agreement.privateKey, owner.agreementPublic, context, made.wraps[at]?.wrap ?? new Uint8Array(0));
      const rescue = await x25519PrivateFromBytes(raw, true);
      const jwk = await crypto.subtle.exportKey("jwk", rescue);
      assert.equal(typeof jwk.x, "string");
    }
  });

  it("is refused when an admin signs it", async () => {
    const [founder, admin] = await Promise.all([person("acc-1"), person("acc-2")]);
    const chain = await chainOf(founder, [admin], admin);
    const vouched = await verifyChain(COMPANY, founder.signer.publicBytes, chain);
    await assert.rejects(newRecoveryKey(COMPANY, admin.signer, founder.signer.publicBytes, chain, vouched, [ownerKey(founder)]), /unvouched-signer/);
  });
});

describe("the handoff code", () => {
  it("is the same for one key, differs between keys, and reads as four groups of four", async () => {
    const one = await publicBytes((await generatePair("X25519")).publicKey);
    const two = await publicBytes((await generatePair("X25519")).publicKey);
    assert.equal(await handoffCode(one), await handoffCode(one.slice()));
    assert.notEqual(await handoffCode(one), await handoffCode(two));
    assert.match(await handoffCode(one), /^[0-9A-F]{4}( [0-9A-F]{4}){3}$/);
  });
});
