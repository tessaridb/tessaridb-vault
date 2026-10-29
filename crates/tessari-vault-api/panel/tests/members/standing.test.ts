// The members page keeps the chain in step with the company: admin standing follows each person's role, and a key the
// chain still vouches for after its holder left is ended where it stands. Every statement is checked against the whole
// chain before it would be sent.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Signer, type Statement } from "../../src/crypto/statements.ts";
import { departedAccounts, leaveStatement, standingOf, standingStatements, vouchStatements } from "../../src/members/standing.ts";
import type { ListedMember } from "../../src/records/share-targets.ts";
import type { DirectoryKey } from "../../src/records/shares-model.ts";
import { refusedWith } from "../crypto/support.ts";

const COMPANY = "company-1";

interface Person {
  signer: Signer;
  key: DirectoryKey;
}

async function person(): Promise<Person> {
  const signing = await generatePair("Ed25519");
  const signingPublic: Bytes = await publicBytes(signing.publicKey);
  const agreementPublic: Bytes = await publicBytes((await generatePair("X25519")).publicKey);
  return { signer: { privateKey: signing.privateKey, publicBytes: signingPublic }, key: { keyId: "11".repeat(16), agreementPublic, signingPublic } };
}

const listed = (account: string, role: ListedMember["role"]): ListedMember => ({ account, name: account, email: `${account}@x.test`, role, active: true });

/** The owner's genesis and `others` vouched for by the owner, as keys alone. */
async function chainOf(owner: Person, others: ReadonlyArray<readonly [string, Person]>): Promise<Statement[]> {
  const chain: Statement[] = [];
  for (const [account, who] of [["owner", owner] as const, ...others]) {
    chain.push(await appendStatement(COMPANY, chain.at(-1), { kind: "member", subject: account, agreementPublic: who.key.agreementPublic, signingPublic: who.key.signingPublic }, owner.signer));
  }
  return chain;
}

describe("admin standing follows the role", () => {
  it("gives an admin standing and takes it from a member, keeping the member's key", async () => {
    const [owner, ada, max] = [await person(), await person(), await person()];
    const chain = await chainOf(owner, [["ada", ada], ["max", max]]);
    const owning = owner.key.signingPublic;
    let vouched = await verifyChain(COMPANY, owning, chain);
    assert.equal(standingOf("owner", "owner", vouched, owning), "matches");
    assert.equal(standingOf("admin", "ada", vouched, owning), "needs-admin");
    assert.equal(standingOf("member", "max", vouched, owning), "matches");

    chain.push(...(await standingStatements(COMPANY, owning, chain, owner.signer, "ada", ada.key, "needs-admin")));
    vouched = await verifyChain(COMPANY, owning, chain);
    assert.equal(standingOf("admin", "ada", vouched, owning), "matches");
    // Now demoted on the server: the chain still says admin until a fresh member statement for the same key.
    assert.equal(standingOf("member", "ada", vouched, owning), "needs-demotion");
    chain.push(...(await standingStatements(COMPANY, owning, chain, ada.signer, "ada", ada.key, "needs-demotion")));
    vouched = await verifyChain(COMPANY, owning, chain);
    assert.equal(standingOf("member", "ada", vouched, owning), "matches");
    assert.deepEqual(vouched.signingKeyOf("ada"), ada.key.signingPublic);
  });

  it("gives an owner without the owner key admin standing", async () => {
    const [owner, oli] = [await person(), await person()];
    const vouched = await verifyChain(COMPANY, owner.key.signingPublic, await chainOf(owner, [["oli", oli]]));
    assert.equal(standingOf("owner", "oli", vouched, owner.key.signingPublic), "needs-admin");
  });

  it("vouches for a waiting admin's key and gives the standing in one step; a member's key alone", async () => {
    const [owner, ada, max] = [await person(), await person(), await person()];
    const owning = owner.key.signingPublic;
    const chain = await chainOf(owner, []);
    const forAda = await vouchStatements(COMPANY, owning, chain, owner.signer, "ada", "admin", ada.key);
    assert.equal(forAda.length, 2);
    const vouched = await verifyChain(COMPANY, owning, [...chain, ...forAda]);
    assert.equal(vouched.hasAdminStanding("ada"), true);
    assert.equal((await vouchStatements(COMPANY, owning, chain, owner.signer, "max", "member", max.key)).length, 1);
    await refusedWith(vouchStatements(COMPANY, owning, chain, max.signer, "max", "member", max.key), "unvouched-signer");
  });
});

describe("a key the chain still vouches for after its holder left", () => {
  it("is listed, ended by a leave, and never the owner key's holder", async () => {
    const [owner, ada, max] = [await person(), await person(), await person()];
    const owning = owner.key.signingPublic;
    const chain = await chainOf(owner, [["ada", ada], ["max", max]]);
    let vouched = await verifyChain(COMPANY, owning, chain);
    assert.deepEqual(departedAccounts([listed("ada", "member")], vouched, owning), ["max"]);
    chain.push(...(await leaveStatement(COMPANY, owning, chain, owner.signer, "max")));
    vouched = await verifyChain(COMPANY, owning, chain);
    assert.deepEqual(departedAccounts([listed("ada", "member")], vouched, owning), []);
    assert.equal(vouched.signingKeyOf("max"), undefined);
    await refusedWith(leaveStatement(COMPANY, owning, chain, ada.signer, "ada"), "unvouched-signer");
  });
});
