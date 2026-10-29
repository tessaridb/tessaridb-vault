// A member's key set: made in the browser under a new unlock password, packed for the server, and opened again only by
// that password, for that member, in that company.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

import { concat, type Bytes } from "../../src/crypto/bytes.ts";
import {
  NEW_KDF,
  createKeySet,
  openKeySet,
  type StoredKeySet,
} from "../../src/crypto/key-set.ts";
import { generatePair, publicBytes, x25519 } from "../../src/crypto/keys.ts";
import { encode } from "../../src/base64url.ts";
import { keySetBody } from "../../src/unlock-model.ts";
import { refusedWith } from "./support.ts";

const WASM = new URL(
  "../../../../../target/wasm32-unknown-unknown/release/tessari_vault_crypto.wasm",
  import.meta.url,
);
const MODULE = new WebAssembly.Module(readFileSync(WASM));
const PASSWORD = "my unlock password, not my sign-in";

const made = createKeySet(MODULE, PASSWORD, "account-1", "company-1", "self");

/** `stored` with its wrapped bytes replaced. */
const withWrapped = (stored: StoredKeySet, wrapped: Bytes): StoredKeySet => ({
  ...stored,
  wrapped,
});

describe("a member's key set", () => {
  it("is made at the module's floor and opens with its unlock password into keys that work", async () => {
    const { stored, open } = await made;
    assert.deepEqual(stored.kdf, NEW_KDF);
    assert.equal(stored.salt.length, 16);
    assert.equal(stored.keyId.length, 16);
    assert.equal(open.agreementPrivate.extractable, false);
    assert.equal(open.signingPrivate.extractable, false);
    const reopened = await openKeySet(
      MODULE,
      PASSWORD,
      "account-1",
      "company-1",
      stored,
    );
    // The agreement key it opens agrees with its stored public half: the same shared secret from both sides.
    const peer = await crypto.subtle.generateKey({ name: "X25519" }, false, [
      "deriveBits",
    ]);
    if (!("privateKey" in peer)) {
      throw new Error("a pair");
    }
    const mine = await x25519(reopened.agreementPrivate, peer.publicKey);
    const theirs = await x25519(
      peer.privateKey,
      await crypto.subtle.importKey(
        "raw",
        stored.agreementPublic,
        { name: "X25519" },
        true,
        [],
      ),
    );
    assert.deepEqual(mine, theirs);
    // The signing key it opens signs for its stored public half.
    const message = new TextEncoder().encode("a revision");
    const signature = await crypto.subtle.sign(
      "Ed25519",
      reopened.signingPrivate,
      message,
    );
    const verifier = await crypto.subtle.importKey(
      "raw",
      stored.signingPublic,
      { name: "Ed25519" },
      true,
      ["verify"],
    );
    assert.equal(
      await crypto.subtle.verify("Ed25519", verifier, signature, message),
      true,
    );
    assert.equal((await publicBytes(verifier)).length, 32);
  });

  it("is sent without its unlock password, and the sign-in password does not open it", async () => {
    // The server holds this body and checks the sign-in password; neither may carry the other's way in.
    const SIGN_IN = "my sign-in password, which the server checks";
    const { stored } = await made;
    const body = JSON.stringify(keySetBody(stored));
    const password = new TextEncoder().encode(PASSWORD);
    for (const form of [
      PASSWORD,
      encode(password),
      Buffer.from(password).toString("base64"),
      Buffer.from(password).toString("hex"),
    ]) {
      assert.equal(
        body.includes(form),
        false,
        `the body carries the unlock password as ${form}`,
      );
    }
    await refusedWith(
      openKeySet(MODULE, SIGN_IN, "account-1", "company-1", stored),
      "unauthentic",
    );
  });

  it("opens for no other password, member or company", async () => {
    const { stored } = await made;
    await refusedWith(
      openKeySet(MODULE, "a wrong password", "account-1", "company-1", stored),
      "unauthentic",
    );
    await refusedWith(
      openKeySet(MODULE, PASSWORD, "account-2", "company-1", stored),
      "unauthentic",
    );
    await refusedWith(
      openKeySet(MODULE, PASSWORD, "account-1", "company-2", stored),
      "unauthentic",
    );
  });

  it("pins its own signing key when its member is the first owner", async () => {
    const { stored, open } = await made;
    assert.deepEqual(open.ownerSigningPublic, stored.signingPublic);
    const reopened = await openKeySet(
      MODULE,
      PASSWORD,
      "account-1",
      "company-1",
      stored,
    );
    assert.deepEqual(reopened.ownerSigningPublic, stored.signingPublic);
  });

  it("pins the owner key it was given, and opens it back", async () => {
    const owner = await publicBytes((await generatePair("Ed25519")).publicKey);
    const { stored, open } = await createKeySet(
      MODULE,
      PASSWORD,
      "account-2",
      "company-1",
      owner,
    );
    assert.deepEqual(open.ownerSigningPublic, owner);
    assert.notDeepEqual(open.ownerSigningPublic, stored.signingPublic);
    const reopened = await openKeySet(
      MODULE,
      PASSWORD,
      "account-2",
      "company-1",
      stored,
    );
    assert.deepEqual(reopened.ownerSigningPublic, owner);
  });

  it("refuses a set whose wraps were swapped, cut or added to", async () => {
    const { stored } = await made;
    const head = stored.wrapped.slice(0, 5);
    const rest = stored.wrapped.slice(5);
    const fieldOf = (bytes: Bytes): Bytes =>
      bytes.slice(
        0,
        4 + new DataView(bytes.buffer, bytes.byteOffset).getUint32(0),
      );
    const first = fieldOf(rest);
    const second = fieldOf(rest.slice(first.length));
    const pin = rest.slice(first.length + second.length);
    await refusedWith(
      openKeySet(
        MODULE,
        PASSWORD,
        "account-1",
        "company-1",
        withWrapped(stored, concat(head, second, first, pin)),
      ),
      "unauthentic",
    );
    await refusedWith(
      openKeySet(
        MODULE,
        PASSWORD,
        "account-1",
        "company-1",
        withWrapped(stored, concat(head, first, second)),
      ),
      "malformed",
    );
    await refusedWith(
      openKeySet(
        MODULE,
        PASSWORD,
        "account-1",
        "company-1",
        withWrapped(stored, concat(stored.wrapped, Uint8Array.of(0))),
      ),
      "malformed",
    );
    await refusedWith(
      openKeySet(
        MODULE,
        PASSWORD,
        "account-1",
        "company-1",
        withWrapped(stored, concat(Uint8Array.of(9), rest)),
      ),
      "unknown-version",
    );
  });

  it("refuses a first-version set, which pinned no owner key", async () => {
    const { stored } = await made;
    const rest = stored.wrapped.slice(1);
    await refusedWith(
      openKeySet(
        MODULE,
        PASSWORD,
        "account-1",
        "company-1",
        withWrapped(stored, concat(Uint8Array.of(1), rest)),
      ),
      "unknown-version",
    );
  });
});
