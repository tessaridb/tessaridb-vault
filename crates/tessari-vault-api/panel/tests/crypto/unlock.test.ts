// The unlock path: the WebAssembly Argon2id module turns the unlock password into the master key, HKDF turns that into
// the unlock key UK, and UK wraps the member's private keys (algorithm 0x03), at most 2^20 times.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

import { random, type Bytes } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes, x25519, x25519PublicFromBytes } from "../../src/crypto/keys.ts";
import { MAX_WRAPS, UnlockKey, deriveMasterKey, type MemberKeyContext } from "../../src/crypto/unlock.ts";
import { flipped, refusedWith, toHex } from "./support.ts";

const WASM = new URL(
  "../../../../../target/wasm32-unknown-unknown/release/tessari_vault_crypto.wasm",
  import.meta.url,
);
const MODULE = new WebAssembly.Module(readFileSync(WASM));
const FLOOR = { memoryKib: 65_536, passes: 3, lanes: 4 };
// The crate's own vector (tessari-vault-crypto unlock_tests.rs), produced by an independent Argon2id implementation.
const FLOOR_VECTOR = "853b272a44db1421c02962669a55eb0994f3cab385ed1c4c79253eee19bab49e";

function context(kind: MemberKeyContext["kind"] = "agreement"): MemberKeyContext {
  return { account: "account-1", company: "company-1", kind, keyId: random(16) };
}

async function unlockKey(wrapsMade = 0): Promise<UnlockKey> {
  return UnlockKey.fromMasterKey(random(32), wrapsMade);
}

describe("master key from the unlock password", () => {
  it("reproduces the independent Argon2id vector at the unlock floor", async () => {
    const salt = Uint8Array.from({ length: 16 }, (_, index) => index);
    const key = await deriveMasterKey(MODULE, "correct horse battery staple", salt, FLOOR);
    assert.equal(toHex(key), FLOOR_VECTOR);
  });

  it("refuses parameters below the floor the module enforces", async () => {
    const salt = random(16);
    await refusedWith(deriveMasterKey(MODULE, "pw", salt, { memoryKib: 19_456, passes: 2, lanes: 1 }), "kdf-refused");
  });
});

describe("member private keys under the unlock key (algorithm 0x03)", () => {
  it("unwraps an agreement key that still agrees with its public half", async () => {
    const uk = await unlockKey();
    const member = await generatePair("X25519");
    const peer = await generatePair("X25519");
    const ctx = context();
    const wrapped = await uk.wrap(member.privateKey, ctx);
    assert.equal(wrapped[0], 1);
    assert.equal(wrapped[1], 0x03);
    const unwrapped = await uk.unwrap(ctx, wrapped);
    assert.equal(unwrapped.extractable, false);
    const viaUnwrapped = await x25519(unwrapped, peer.publicKey);
    const viaPeer = await x25519(peer.privateKey, await x25519PublicFromBytes(await publicBytes(member.publicKey)));
    assert.equal(toHex(viaUnwrapped), toHex(viaPeer));
  });

  it("unwraps a signing key that signs for its public half", async () => {
    const uk = await unlockKey();
    const member = await generatePair("Ed25519");
    const ctx = context("signing");
    const unwrapped = await uk.unwrap(ctx, await uk.wrap(member.privateKey, ctx));
    const message = random(40);
    const signature = await crypto.subtle.sign("Ed25519", unwrapped, message);
    assert.ok(await crypto.subtle.verify("Ed25519", member.publicKey, signature, message));
  });

  it("refuses a wrap read for another account, company or key kind", async () => {
    const uk = await unlockKey();
    const ctx = context();
    const wrapped = await uk.wrap((await generatePair("X25519")).privateKey, ctx);
    for (const other of [
      { ...ctx, account: "account-2" },
      { ...ctx, company: "company-2" },
      { ...ctx, kind: "signing" as const },
    ]) {
      await refusedWith(uk.unwrap(other, wrapped), "unauthentic");
    }
    await refusedWith(uk.unwrap({ ...ctx, keyId: random(16) }, wrapped), "key-mismatch");
    await refusedWith(uk.unwrap(ctx, flipped(wrapped, wrapped.length - 1)), "unauthentic");
  });

  it("refuses a wrap under another unlock key", async () => {
    const ctx = context();
    const wrapped = await (await unlockKey()).wrap((await generatePair("X25519")).privateKey, ctx);
    await refusedWith((await unlockKey()).unwrap(ctx, wrapped), "unauthentic");
  });

  it("refuses an unknown version or algorithm", async () => {
    const uk = await unlockKey();
    const ctx = context();
    const wrapped: Bytes = await uk.wrap((await generatePair("X25519")).privateKey, ctx);
    const version = wrapped.slice();
    version[0] = 9;
    await refusedWith(uk.unwrap(ctx, version), "unknown-version");
    const algorithm = wrapped.slice();
    algorithm[1] = 0x01;
    await refusedWith(uk.unwrap(ctx, algorithm), "unknown-algorithm");
  });

  it("makes the last permitted wrap and refuses the next one (NT-18)", async () => {
    const uk = await unlockKey(MAX_WRAPS - 1);
    const key = (await generatePair("X25519")).privateKey;
    await uk.wrap(key, context());
    assert.equal(uk.wrapsMade, MAX_WRAPS);
    await refusedWith(uk.wrap(key, context()), "wrap-bound");
    assert.equal(uk.wrapsMade, MAX_WRAPS);
  });

  it("refuses to be opened over a count already past the bound", async () => {
    await refusedWith(unlockKey(MAX_WRAPS + 1), "wrap-bound");
  });
});

describe("the owner key pinned under the unlock key", () => {
  const pin = { account: "account-1", company: "company-1", keyId: random(16) };

  it("opens back into the owner's public key and counts as one wrap", async () => {
    const uk = await unlockKey();
    const owner = await generatePair("Ed25519");
    const { nonce, sealed } = await uk.sealPin(owner.publicKey, pin);
    assert.equal(uk.wrapsMade, 1);
    const opened = await uk.openPin(pin, nonce, sealed);
    assert.equal(toHex(await publicBytes(opened)), toHex(await publicBytes(owner.publicKey)));
  });

  it("refuses a pin read for another account, company or key set, or with a changed byte", async () => {
    const uk = await unlockKey();
    const { nonce, sealed } = await uk.sealPin((await generatePair("Ed25519")).publicKey, pin);
    await refusedWith(uk.openPin({ ...pin, account: "account-2" }, nonce, sealed), "unauthentic");
    await refusedWith(uk.openPin({ ...pin, company: "company-2" }, nonce, sealed), "unauthentic");
    await refusedWith(uk.openPin({ ...pin, keyId: random(16) }, nonce, sealed), "unauthentic");
    await refusedWith(uk.openPin(pin, nonce, flipped(sealed, 0)), "unauthentic");
    await refusedWith(uk.openPin(pin, nonce.slice(1), sealed), "malformed");
  });

  it("is not made past the wrap bound", async () => {
    const uk = await unlockKey(MAX_WRAPS);
    await refusedWith(uk.sealPin((await generatePair("Ed25519")).publicKey, pin), "wrap-bound");
  });
});
