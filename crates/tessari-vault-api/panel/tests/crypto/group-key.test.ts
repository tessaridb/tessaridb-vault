// Keying a group in an admin's browser: a new X25519 pair, a chain statement vouching for its public half, and its
// private half wrapped to each member the chain vouches for. A member opens the wrap the way records are opened; a
// member the chain does not vouch for yet gets no wrap and stays pending.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Bytes } from "../../src/crypto/bytes.ts";
import type { OpenKeySet } from "../../src/crypto/key-set.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Signer, type Statement } from "../../src/crypto/statements.ts";
import { newGroupKey, pendingWraps } from "../../src/groups/group-key.ts";
import { groupKey } from "../../src/records/recipient-keys.ts";
import { noise, refusedWith, toHex } from "./support.ts";

const COMPANY = "c".repeat(32);
const TEAM = "d".repeat(32);

interface Person {
  readonly account: string;
  readonly keys: OpenKeySet;
  readonly signer: Signer;
}

async function person(account: string, owner?: Bytes): Promise<Person> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  const signingPublic = await publicBytes(signing.publicKey);
  const keys: OpenKeySet = {
    keyId: noise(16),
    agreementPrivate: agreement.privateKey,
    signingPrivate: signing.privateKey,
    agreementPublic: await publicBytes(agreement.publicKey),
    signingPublic,
    ownerSigningPublic: owner ?? signingPublic,
  };
  return { account, keys, signer: { privateKey: signing.privateKey, publicBytes: signingPublic } };
}

const memberStatement = (company: string, previous: Statement | undefined, who: Person, signer: Signer): Promise<Statement> =>
  appendStatement(company, previous, { kind: "member", subject: who.account, agreementPublic: who.keys.agreementPublic, signingPublic: who.keys.signingPublic }, signer);

describe("keying a group", () => {
  it("vouches for the new key and wraps it to each vouched member, who can open it", async () => {
    const owner = await person("a".repeat(32));
    const alice = await person("b".repeat(32), owner.keys.signingPublic);
    const stranger = await person("e".repeat(32), owner.keys.signingPublic);
    const chain: Statement[] = [await memberStatement(COMPANY, undefined, owner, owner.signer)];
    chain.push(await memberStatement(COMPANY, chain.at(-1), alice, owner.signer));
    const before = await verifyChain(COMPANY, owner.keys.signingPublic, chain);
    const pending = [alice, stranger].map((who) => ({ account: who.account, keyId: who.keys.keyId, agreementPublic: who.keys.agreementPublic }));

    const made = await newGroupKey(COMPANY, TEAM, owner.signer, owner.keys.signingPublic, chain, before, pending);

    assert.equal(made.keyId.length, 16);
    assert.deepEqual(made.wraps.map((each) => toHex(each.keyId)), [toHex(alice.keys.keyId)]);
    assert.deepEqual(made.unvouched, [stranger.account]);
    const after = await verifyChain(COMPANY, owner.keys.signingPublic, [...chain, made.statement]);
    assert.equal(toHex(after.groupKeyOf(TEAM) ?? new Uint8Array()), toHex(made.public));

    const wrap = made.wraps[0]?.wrap ?? new Uint8Array();
    const opened = await groupKey(COMPANY, after, alice.keys, TEAM, { keyId: made.keyId, public: made.public, wrap });
    assert.equal(opened.kind, "group");
    assert.equal(toHex(opened.agreementPublic), toHex(made.public));
  });

  it("lets a member who holds the key give it to a member vouched since, who can then open it", async () => {
    const owner = await person("a".repeat(32));
    const alice = await person("b".repeat(32), owner.keys.signingPublic);
    const bob = await person("e".repeat(32), owner.keys.signingPublic);
    const chain: Statement[] = [await memberStatement(COMPANY, undefined, owner, owner.signer)];
    chain.push(await memberStatement(COMPANY, chain.at(-1), alice, owner.signer));
    const pendingOf = (who: Person) => ({ account: who.account, keyId: who.keys.keyId, agreementPublic: who.keys.agreementPublic });
    const made = await newGroupKey(COMPANY, TEAM, owner.signer, owner.keys.signingPublic, chain, await verifyChain(COMPANY, owner.keys.signingPublic, chain), [pendingOf(alice), pendingOf(bob)]);
    chain.push(made.statement);
    chain.push(await memberStatement(COMPANY, chain.at(-1), bob, owner.signer));
    const now = await verifyChain(COMPANY, owner.keys.signingPublic, chain);
    const alicesWrap = made.wraps[0]?.wrap ?? new Uint8Array();

    const given = await pendingWraps(COMPANY, TEAM, alice.keys, now, { keyId: made.keyId, public: made.public, wrap: alicesWrap }, [pendingOf(bob)]);

    assert.deepEqual(given.unvouched, []);
    const bobsWrap = given.wraps[0]?.wrap ?? new Uint8Array();
    const opened = await groupKey(COMPANY, now, bob.keys, TEAM, { keyId: made.keyId, public: made.public, wrap: bobsWrap });
    assert.equal(toHex(opened.agreementPublic), toHex(made.public));

    // A server that lists a key of its own for the group gets nothing wrapped to anyone.
    await refusedWith(pendingWraps(COMPANY, TEAM, alice.keys, now, { keyId: made.keyId, public: noise(32), wrap: alicesWrap }, [pendingOf(bob)]), "unvouched-recipient");
  });
});
