// Escrow (C7): the member's private keys leave the browser only wrapped to the company recovery key, and come back to
// the member after a recovery, who keeps the same key set under a new unlock password.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

import { concat } from "../../src/crypto/bytes.ts";
import { readEscrow } from "../../src/crypto/escrow.ts";
import { createKeySet, openKeySet, openKeySetWithEscrow, restoreKeySet, type OpenKeySet } from "../../src/crypto/key-set.ts";
import { x25519 } from "../../src/crypto/keys.ts";
import { refusedWith } from "./support.ts";

const WASM = new URL("../../../../../target/wasm32-unknown-unknown/release/tessari_vault_crypto.wasm", import.meta.url);
const MODULE = new WebAssembly.Module(readFileSync(WASM));
const OLD = "the unlock password I forgot";
const NEW = "the unlock password I set now";

const made = createKeySet(MODULE, OLD, "account-1", "company-1", "self", true);

/** A copy of an escrow: restoring wipes the one it is given. */
const copy = (escrow: Uint8Array<ArrayBuffer> | null): Uint8Array<ArrayBuffer> => (escrow ?? new Uint8Array(0)).slice();

/** The secret `keys` agree on with a fresh peer, from both sides: equal only when the private half matches. */
async function agrees(keys: OpenKeySet): Promise<boolean> {
  const peer = await crypto.subtle.generateKey({ name: "X25519" }, false, ["deriveBits"]);
  if (!("privateKey" in peer)) {
    throw new Error("a pair");
  }
  const mine = await x25519(keys.agreementPrivate, peer.publicKey);
  const theirs = await x25519(peer.privateKey, await crypto.subtle.importKey("raw", keys.agreementPublic, { name: "X25519" }, true, []));
  return mine.every((byte, at) => byte === theirs[at]);
}

describe("escrow of a member's keys", () => {
  it("is made with a new key set, and again when the key set is unlocked for it", async () => {
    const { stored, escrow } = await made;
    assert.notEqual(escrow, null);
    const opened = await openKeySetWithEscrow(MODULE, OLD, "account-1", "company-1", stored);
    for (const each of [escrow, opened.escrow]) {
      assert.deepEqual(readEscrow(each ?? new Uint8Array(0)).ownerSigningPublic, stored.signingPublic);
    }
    assert.equal(opened.open.agreementPrivate.extractable, false);
  });

  it("restores the same keys under a new unlock password, with the owner key still pinned", async () => {
    const { stored, escrow } = await made;
    const given = copy(escrow);
    const restored = await restoreKeySet(MODULE, NEW, "account-1", "company-1", stored, given);
    assert.ok(given.every((byte) => byte === 0), "the escrow handed in is wiped");
    assert.deepEqual(restored.stored.keyId, stored.keyId);
    assert.deepEqual(restored.stored.agreementPublic, stored.agreementPublic);
    assert.deepEqual(restored.stored.signingPublic, stored.signingPublic);
    assert.notDeepEqual(restored.stored.salt, stored.salt);
    const reopened = await openKeySet(MODULE, NEW, "account-1", "company-1", restored.stored);
    assert.ok(await agrees(reopened));
    assert.deepEqual(reopened.ownerSigningPublic, stored.signingPublic);
    await refusedWith(openKeySet(MODULE, OLD, "account-1", "company-1", restored.stored), "unauthentic");
  });

  it("refuses keys that are not the member's own key set", async () => {
    const { escrow } = await made;
    const other = await createKeySet(MODULE, OLD, "account-2", "company-1", "self");
    await refusedWith(restoreKeySet(MODULE, NEW, "account-1", "company-1", other.stored, copy(escrow)), "key-mismatch");
  });

  it("refuses a damaged escrow", async () => {
    const { escrow } = await made;
    const bytes = copy(escrow);
    assert.throws(() => readEscrow(bytes.slice(0, bytes.length - 1)), /malformed/);
    assert.throws(() => readEscrow(concat(Uint8Array.of(9), bytes.slice(1))), /unknown-version/);
  });
});
