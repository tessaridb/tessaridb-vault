// NT-33: the browser wraps a key only to a key the owner's chain vouches for. The server lists members' and groups'
// public keys, but a key it lists and the chain does not vouch is refused, so the server cannot add a reader.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { wrapToVouched, type Recipient } from "../../src/crypto/recipients.ts";
import { appendStatement, verifyChain, type Signer, type Statement, type StatementFields, type Vouched } from "../../src/crypto/statements.ts";
import { unwrapSecret, type WrapContext } from "../../src/crypto/wraps.ts";
import { noise, refusedWith, toHex } from "./support.ts";

const COMPANY = "company-1";

interface Keys {
  signer: Signer;
  agreement: CryptoKeyPair;
  agreementPublic: Bytes;
}

async function keys(): Promise<Keys> {
  const signing = await generatePair("Ed25519");
  const agreement = await generatePair("X25519");
  return {
    signer: { privateKey: signing.privateKey, publicBytes: await publicBytes(signing.publicKey) },
    agreement,
    agreementPublic: await publicBytes(agreement.publicKey),
  };
}

function member(account: string, who: Keys): StatementFields {
  return { kind: "member", subject: account, agreementPublic: who.agreementPublic, signingPublic: who.signer.publicBytes };
}

/** The owner's genesis, alice vouched, a team key vouched; `then` adds statements after. */
async function vouched(then: (add: (fields: StatementFields) => Promise<void>) => Promise<void> = async () => {}): Promise<{
  alice: Keys;
  team: CryptoKeyPair;
  teamPublic: Bytes;
  rescue: CryptoKeyPair;
  rescuePublic: Bytes;
  chain: Vouched;
}> {
  const owner = await keys();
  const alice = await keys();
  const team = await generatePair("X25519");
  const teamPublic = await publicBytes(team.publicKey);
  const rescue = await generatePair("X25519");
  const rescuePublic = await publicBytes(rescue.publicKey);
  const statements: Statement[] = [];
  const add = async (fields: StatementFields): Promise<void> => {
    statements.push(await appendStatement(COMPANY, statements.at(-1), fields, owner.signer));
  };
  await add(member("owner", owner));
  await add(member("alice", alice));
  await add({ kind: "group", subject: "team-1", agreementPublic: teamPublic, signingPublic: new Uint8Array(0) });
  await add({ kind: "recovery", subject: "cc".repeat(16), agreementPublic: rescuePublic, signingPublic: new Uint8Array(0) });
  await then(add);
  return { alice, team, teamPublic, rescue, rescuePublic, chain: await verifyChain(COMPANY, owner.signer.publicBytes, statements) };
}

function context(recipientKind: "member" | "group" | "recovery"): WrapContext {
  return { company: COMPANY, wrappedKind: "record", wrappedKeyId: noise(16), recipientKind, recipientKeyId: noise(16) };
}

const ALICE: Recipient = { kind: "member", account: "alice" };
const TEAM: Recipient = { kind: "group", group: "team-1" };
const RESCUE: Recipient = { kind: "recovery" };

describe("wrapping to a recipient", () => {
  it("wraps to the member key and the group key the chain vouches for", async () => {
    const { alice, team, teamPublic, chain } = await vouched();
    const secret = noise(32);
    const toAlice = context("member");
    const wrap = await wrapToVouched(chain, ALICE, alice.agreementPublic, toAlice, secret);
    assert.equal(toHex(await unwrapSecret(alice.agreement.privateKey, alice.agreementPublic, toAlice, wrap)), toHex(secret));
    const toTeam = context("group");
    const teamWrap = await wrapToVouched(chain, TEAM, teamPublic, toTeam, secret);
    assert.equal(toHex(await unwrapSecret(team.privateKey, teamPublic, toTeam, teamWrap)), toHex(secret));
  });

  it("wraps a member's keys to the recovery key the owner vouched, and to no other", async () => {
    const { rescue, rescuePublic, chain } = await vouched();
    const secret = noise(32);
    const toRescue = context("recovery");
    const wrap = await wrapToVouched(chain, RESCUE, rescuePublic, toRescue, secret);
    assert.equal(toHex(await unwrapSecret(rescue.privateKey, rescuePublic, toRescue, wrap)), toHex(secret));
    const substituted = await publicBytes((await generatePair("X25519")).publicKey);
    await refusedWith(wrapToVouched(chain, RESCUE, substituted, toRescue, secret), "unvouched-recipient");
  });

  it("refuses a key the server lists in place of the vouched one", async () => {
    const { chain } = await vouched();
    const substituted = await publicBytes((await generatePair("X25519")).publicKey);
    await refusedWith(wrapToVouched(chain, ALICE, substituted, context("member"), noise(32)), "unvouched-recipient");
    await refusedWith(wrapToVouched(chain, TEAM, substituted, context("group"), noise(32)), "unvouched-recipient");
  });

  it("refuses a member or group the chain never vouched, or revoked", async () => {
    const stranger = await publicBytes((await generatePair("X25519")).publicKey);
    const { chain } = await vouched();
    await refusedWith(
      wrapToVouched(chain, { kind: "member", account: "mallory" }, stranger, context("member"), noise(32)),
      "unvouched-recipient",
    );
    await refusedWith(
      wrapToVouched(chain, { kind: "group", group: "team-2" }, stranger, context("group"), noise(32)),
      "unvouched-recipient",
    );
    const revoked = await vouched(async (add) => {
      await add({ kind: "revoke", subject: "alice", agreementPublic: new Uint8Array(0), signingPublic: new Uint8Array(0) });
    });
    await refusedWith(
      wrapToVouched(revoked.chain, ALICE, revoked.alice.agreementPublic, context("member"), noise(32)),
      "unvouched-recipient",
    );
  });

  it("refuses a context naming another kind of recipient", async () => {
    const { alice, chain } = await vouched();
    await refusedWith(wrapToVouched(chain, ALICE, alice.agreementPublic, context("group"), noise(32)), "malformed");
  });
});
