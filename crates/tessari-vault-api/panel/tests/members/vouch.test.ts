// Vouching for a member's key from the members page: the fingerprint two people compare, each member's key state, and
// the statement an owner or admin appends, checked against the whole chain before it is sent.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { random, type Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { appendStatement, verifyChain, type Statement } from "../../src/crypto/statements.ts";
import { keyFingerprint } from "../../src/crypto/fingerprint.ts";
import { memberRows } from "../../src/members/members-model.ts";
import { vouchStatements } from "../../src/members/standing.ts";
import type { ListedMember } from "../../src/records/share-targets.ts";
import type { DirectoryKey } from "../../src/records/shares-model.ts";
import { refusedWith } from "../crypto/support.ts";

const COMPANY = "company-1";
const [OWNER, SAM, NEW] = ["a", "b", "c"].map((c) => c.repeat(32)) as [string, string, string];

async function person(): Promise<{ signing: CryptoKeyPair; signingPublic: Bytes; agreementPublic: Bytes }> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  return { signing, signingPublic: await publicBytes(signing.publicKey), agreementPublic: await publicBytes(agreement.publicKey) };
}

describe("a key's fingerprint", () => {
  it("is eight groups of four hex digits that change with either key", async () => {
    const [a, b] = [random(32), random(32)];
    const shown = await keyFingerprint(a, b);
    assert.match(shown, /^([0-9A-F]{4} ){7}[0-9A-F]{4}$/);
    assert.equal(await keyFingerprint(a, b), shown);
    assert.notEqual(await keyFingerprint(random(32), b), shown);
    assert.notEqual(await keyFingerprint(a, random(32)), shown);
    assert.notEqual(await keyFingerprint(b, a), shown);
  });
});

describe("vouching for a member's key", () => {
  it("appends a member statement the whole chain still verifies, and says each member's key state", async () => {
    const [owner, sam] = [await person(), await person()];
    const genesis = await appendStatement(COMPANY, undefined, { kind: "member", subject: OWNER, agreementPublic: owner.agreementPublic, signingPublic: owner.signingPublic }, {
      privateKey: owner.signing.privateKey,
      publicBytes: owner.signingPublic,
    });
    const chain: Statement[] = [genesis];
    const directory = new Map<string, DirectoryKey>([
      [OWNER, { keyId: "11".repeat(16), agreementPublic: owner.agreementPublic, signingPublic: owner.signingPublic }],
      [SAM, { keyId: "22".repeat(16), agreementPublic: sam.agreementPublic, signingPublic: sam.signingPublic }],
    ]);
    const members: ListedMember[] = [
      { account: OWNER, name: "Olive Owner", email: "owner@x.test", role: "owner", active: true },
      { account: SAM, name: "Sam Member", email: "sam@x.test", role: "member", active: true },
      { account: NEW, name: "Ned New", email: "new@x.test", role: "member", active: true },
    ];
    const before = await memberRows(members, directory, await verifyChain(COMPANY, owner.signingPublic, chain));
    assert.deepEqual(before.map((row) => [row.label, row.state]), [["Olive Owner", "vouched"], ["Sam Member", "waiting"], ["Ned New", "no-key"]]);
    assert.equal(before[1]?.fingerprint, await keyFingerprint(sam.agreementPublic, sam.signingPublic));

    const statements = await vouchStatements(COMPANY, owner.signingPublic, chain, { privateKey: owner.signing.privateKey, publicBytes: owner.signingPublic }, SAM, "member", directory.get(SAM) as DirectoryKey);
    const after = await memberRows(members, directory, await verifyChain(COMPANY, owner.signingPublic, [...chain, ...statements]));
    assert.equal(after[1]?.state, "vouched");

    // A member nobody made admin signs a statement the chain would refuse: it is refused here, before anything is sent.
    await refusedWith(
      vouchStatements(COMPANY, owner.signingPublic, chain, { privateKey: sam.signing.privateKey, publicBytes: sam.signingPublic }, SAM, "member", directory.get(SAM) as DirectoryKey),
      "unvouched-signer",
    );
  });

  it("keeps a suspended member on the list, marked, so suspending someone does not make them vanish", async () => {
    const owner = await person();
    const genesis = await appendStatement(COMPANY, undefined, { kind: "member", subject: OWNER, agreementPublic: owner.agreementPublic, signingPublic: owner.signingPublic }, {
      privateKey: owner.signing.privateKey,
      publicBytes: owner.signingPublic,
    });
    const members: ListedMember[] = [
      { account: OWNER, name: "Olive Owner", email: "owner@x.test", role: "owner", active: true },
      { account: SAM, name: "Sam Member", email: "sam@x.test", role: "member", active: false },
    ];
    const rows = await memberRows(members, new Map(), await verifyChain(COMPANY, owner.signingPublic, [genesis]));
    assert.deepEqual(rows.map((row) => [row.label, row.suspended]), [["Olive Owner", false], ["Sam Member", true]]);
  });
});
