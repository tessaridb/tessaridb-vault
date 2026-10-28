// Layer 3: the company owner's key vouches for every member, admin and group key through an ordered, signed statement
// chain, and a client trusts a revision only from a key that chain vouches for. The server stores the chain; it cannot
// add a reader or an author by writing a row, because it cannot sign.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { utf8, type Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { signRevision, verifyRevision, type RevisionContext } from "../../src/crypto/record.ts";
import {
  appendStatement,
  verifyChain,
  type Signer,
  type Statement,
  type StatementFields,
} from "../../src/crypto/statements.ts";
import { flipped, refusedWith, toHex } from "./support.ts";

const COMPANY = "company-1";

interface Person {
  account: string;
  signer: Signer;
  agreementPublic: Bytes;
}

async function person(account: string): Promise<Person> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  return {
    account,
    signer: { privateKey: signing.privateKey, publicBytes: await publicBytes(signing.publicKey) },
    agreementPublic: await publicBytes(agreement.publicKey),
  };
}

function member(who: Person): StatementFields {
  return { kind: "member", subject: who.account, agreementPublic: who.agreementPublic, signingPublic: who.signer.publicBytes };
}

function bare(kind: "admin" | "revoke", subject: string): StatementFields {
  return { kind, subject, agreementPublic: new Uint8Array(0), signingPublic: new Uint8Array(0) };
}

/** Owner genesis, members alice and bob, bob made admin, carol vouched by bob, and one group key. */
async function world(): Promise<{ owner: Person; alice: Person; bob: Person; carol: Person; dave: Person; group: Bytes; chain: Statement[] }> {
  const [owner, alice, bob, carol, dave] = await Promise.all(
    ["owner", "alice", "bob", "carol", "dave"].map((name) => person(name)),
  ) as [Person, Person, Person, Person, Person];
  const group = await publicBytes((await generatePair("X25519")).publicKey);
  const chain: Statement[] = [];
  const add = async (fields: StatementFields, signer: Signer): Promise<void> => {
    chain.push(await appendStatement(COMPANY, chain.at(-1), fields, signer));
  };
  await add(member(owner), owner.signer);
  await add(member(alice), owner.signer);
  await add(member(bob), owner.signer);
  await add(bare("admin", "bob"), owner.signer);
  await add(member(carol), bob.signer);
  await add({ kind: "group", subject: "group-1", agreementPublic: group, signingPublic: new Uint8Array(0) }, owner.signer);
  return { owner, alice, bob, carol, dave, group, chain };
}

const REVISION: RevisionContext = { company: COMPANY, record: "record-1", revision: 3 };
const BODY = utf8("sealed body bytes");

describe("the vouching chain", () => {
  it("vouches for every member, admin-vouched member and group it names", async () => {
    const { owner, alice, carol, group, chain } = await world();
    const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, chain);
    assert.equal(toHex(vouched.signingKeyOf("alice") ?? new Uint8Array()), toHex(alice.signer.publicBytes));
    assert.equal(toHex(vouched.agreementKeyOf("carol") ?? new Uint8Array()), toHex(carol.agreementPublic));
    assert.equal(toHex(vouched.groupKeyOf("group-1") ?? new Uint8Array()), toHex(group));
    assert.equal(vouched.signingKeyOf("dave"), undefined);
  });

  it("drops a revoked member's keys", async () => {
    const { owner, chain } = await world();
    chain.push(await appendStatement(COMPANY, chain.at(-1), bare("revoke", "alice"), owner.signer));
    const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, chain);
    assert.equal(vouched.agreementKeyOf("alice"), undefined);
    assert.equal(vouched.signingKeyOf("alice"), undefined);
  });

  it("refuses a statement signed by a member who is not an admin", async () => {
    const { owner, alice, dave, chain } = await world();
    chain.push(await appendStatement(COMPANY, chain.at(-1), member(dave), alice.signer));
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, chain), "unvouched-signer");
  });

  it("refuses a statement signed by an admin after that admin was revoked", async () => {
    const { owner, bob, dave, chain } = await world();
    chain.push(await appendStatement(COMPANY, chain.at(-1), bare("revoke", "bob"), owner.signer));
    chain.push(await appendStatement(COMPANY, chain.at(-1), member(dave), bob.signer));
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, chain), "unvouched-signer");
  });

  it("refuses a chain with a statement missing, out of order, or from another company", async () => {
    const { owner, chain } = await world();
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, [...chain.slice(0, 2), ...chain.slice(3)]), "chain-broken");
    const swapped = [...chain];
    [swapped[1], swapped[2]] = [chain[2] as Statement, chain[1] as Statement];
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, swapped), "chain-broken");
    await refusedWith(verifyChain("company-2", owner.signer.publicBytes, chain), "chain-broken");
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, []), "chain-broken");
  });

  it("refuses a statement from a fork: the right position, another predecessor", async () => {
    const { owner, dave, chain } = await world();
    const forkSecond = await appendStatement(COMPANY, chain[0], member(dave), owner.signer);
    // Valid at position 3 of the real chain in every respect but its predecessor.
    const forkThird = await appendStatement(COMPANY, forkSecond, bare("revoke", "alice"), owner.signer);
    await refusedWith(
      verifyChain(COMPANY, owner.signer.publicBytes, [chain[0] as Statement, chain[1] as Statement, forkThird]),
      "chain-broken",
    );
  });

  it("refuses a chain whose genesis is not the company owner's key", async () => {
    const { owner, alice, chain } = await world();
    await refusedWith(verifyChain(COMPANY, alice.signer.publicBytes, chain), "chain-broken");
    const forged = [await appendStatement(COMPANY, undefined, member(alice), alice.signer)];
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, forged), "chain-broken");
  });

  it("refuses a statement whose bytes or signature were changed", async () => {
    const { owner, chain } = await world();
    const third = chain[2] as Statement;
    const changedBytes = [...chain];
    changedBytes[2] = { bytes: flipped(third.bytes, third.bytes.length - 40), signature: third.signature };
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, changedBytes), "bad-signature");
    const changedSignature = [...chain];
    changedSignature[2] = { bytes: third.bytes, signature: flipped(third.signature, 10) };
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, changedSignature), "bad-signature");
    const otherAlgorithm = [...chain];
    const signature = third.signature.slice();
    signature[1] = 0x01;
    otherAlgorithm[2] = { bytes: third.bytes, signature };
    await refusedWith(verifyChain(COMPANY, owner.signer.publicBytes, otherAlgorithm), "unknown-algorithm");
  });
});

describe("revision signatures (algorithm 0x04)", () => {
  it("accepts a revision signed by its vouched author", async () => {
    const { owner, alice, chain } = await world();
    const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, chain);
    const signature = await signRevision(alice.signer.privateKey, REVISION, BODY);
    await verifyRevision(vouched, "alice", REVISION, BODY, signature);
  });

  it("refuses a revision by a key the chain does not vouch for", async () => {
    const { owner, dave, chain } = await world();
    const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, chain);
    const signature = await signRevision(dave.signer.privateKey, REVISION, BODY);
    await refusedWith(verifyRevision(vouched, "dave", REVISION, BODY, signature), "unvouched-signer");
  });

  it("refuses a revision by an author revoked since", async () => {
    const { owner, alice, chain } = await world();
    const signature = await signRevision(alice.signer.privateKey, REVISION, BODY);
    chain.push(await appendStatement(COMPANY, chain.at(-1), bare("revoke", "alice"), owner.signer));
    const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, chain);
    await refusedWith(verifyRevision(vouched, "alice", REVISION, BODY, signature), "unvouched-signer");
  });

  it("refuses a revision claimed for another author, body, record, revision or company", async () => {
    const { owner, alice, chain } = await world();
    const vouched = await verifyChain(COMPANY, owner.signer.publicBytes, chain);
    const signature = await signRevision(alice.signer.privateKey, REVISION, BODY);
    await refusedWith(verifyRevision(vouched, "carol", REVISION, BODY, signature), "bad-signature");
    await refusedWith(verifyRevision(vouched, "alice", REVISION, utf8("another body"), signature), "bad-signature");
    for (const moved of [
      { ...REVISION, record: "record-2" },
      { ...REVISION, revision: 4 },
      { ...REVISION, company: "company-2" },
    ]) {
      await refusedWith(verifyRevision(vouched, "alice", moved, BODY, signature), "bad-signature");
    }
  });
});
