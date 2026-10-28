// Known answers for the primitives the module builds on: HKDF (RFC 5869 A.1-A.3), X25519 (RFC 7748 §6.1) and Ed25519
// (RFC 8032 §7.1 tests 1-3). The values are copied from the RFC texts retrieved from rfc-editor.org on 2026-10-28.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { expand, extract } from "../../src/crypto/hkdf.ts";
import {
  ed25519PrivateFromSeed,
  ed25519PublicFromBytes,
  x25519,
  x25519PrivateFromBytes,
  x25519PublicFromBytes,
} from "../../src/crypto/keys.ts";
import { fromHex, refusedWith, toHex } from "./support.ts";

const HKDF_CASES = [
  {
    name: "A.1 basic",
    ikm: "0b".repeat(22),
    salt: "000102030405060708090a0b0c",
    info: "f0f1f2f3f4f5f6f7f8f9",
    length: 42,
    prk: "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5",
    okm: "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865",
  },
  {
    name: "A.2 longer inputs and outputs",
    ikm: Array.from({ length: 80 }, (_, index) => index.toString(16).padStart(2, "0")).join(""),
    salt: Array.from({ length: 80 }, (_, index) => (0x60 + index).toString(16).padStart(2, "0")).join(""),
    info: Array.from({ length: 80 }, (_, index) => (0xb0 + index).toString(16).padStart(2, "0")).join(""),
    length: 82,
    prk: "06a6b88c5853361a06104c9ceb35b45cef760014904671014a193f40c15fc244",
    okm:
      "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c59045a99cac7827271cb41c65e590e09" +
      "da3275600c2f09b8367793a9aca3db71cc30c58179ec3e87c14c01d5c1f3434f1d87",
  },
  {
    name: "A.3 zero-length salt and info",
    ikm: "0b".repeat(22),
    salt: "",
    info: "",
    length: 42,
    prk: "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04",
    okm: "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8",
  },
];

describe("HKDF-SHA256 (RFC 5869)", () => {
  for (const vector of HKDF_CASES) {
    it(`reproduces test case ${vector.name}`, async () => {
      const prk = await extract(fromHex(vector.salt), fromHex(vector.ikm));
      assert.equal(toHex(prk), vector.prk);
      const okm = await expand(prk, fromHex(vector.info), vector.length);
      assert.equal(toHex(okm), vector.okm);
    });
  }
});

const ALICE_PRIVATE = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a";
const ALICE_PUBLIC = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";
const BOB_PRIVATE = "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb";
const BOB_PUBLIC = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f";
const SHARED = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742";

describe("X25519 (RFC 7748 §6.1)", () => {
  it("derives each party's public key from its private key", async () => {
    for (const [privateHex, publicHex] of [
      [ALICE_PRIVATE, ALICE_PUBLIC],
      [BOB_PRIVATE, BOB_PUBLIC],
    ] as const) {
      const key = await x25519PrivateFromBytes(fromHex(privateHex), true);
      const jwk = await crypto.subtle.exportKey("jwk", key);
      assert.equal(toHex(Buffer.from(jwk.x ?? "", "base64url")), publicHex);
    }
  });

  it("agrees on the shared secret from both sides", async () => {
    const alice = await x25519PrivateFromBytes(fromHex(ALICE_PRIVATE), false);
    const bob = await x25519PrivateFromBytes(fromHex(BOB_PRIVATE), false);
    assert.equal(toHex(await x25519(alice, await x25519PublicFromBytes(fromHex(BOB_PUBLIC)))), SHARED);
    assert.equal(toHex(await x25519(bob, await x25519PublicFromBytes(fromHex(ALICE_PUBLIC)))), SHARED);
  });

  it("refuses a small-order peer key whose shared secret is all zero", async () => {
    const alice = await x25519PrivateFromBytes(fromHex(ALICE_PRIVATE), false);
    const zeroPoint = await x25519PublicFromBytes(new Uint8Array(32));
    await refusedWith(x25519(alice, zeroPoint), "weak-shared-secret");
  });
});

const ED25519_CASES = [
  {
    name: "1 (empty message)",
    seed: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
    publicKey: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
    message: "",
    signature:
      "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46b" +
      "d25bf5f0595bbe24655141438e7a100b",
  },
  {
    name: "2 (one byte)",
    seed: "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
    publicKey: "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
    message: "72",
    signature:
      "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c" +
      "387b2eaeb4302aeeb00d291612bb0c00",
  },
  {
    name: "3 (two bytes)",
    seed: "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
    publicKey: "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
    message: "af82",
    signature:
      "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc659" +
      "4a7c15e9716ed28dc027beceea1ec40a",
  },
];

describe("Ed25519 (RFC 8032 §7.1)", () => {
  for (const vector of ED25519_CASES) {
    it(`signs and verifies test ${vector.name}`, async () => {
      const signer = await ed25519PrivateFromSeed(fromHex(vector.seed), false);
      const signature = new Uint8Array(
        await crypto.subtle.sign("Ed25519", signer, fromHex(vector.message)),
      );
      assert.equal(toHex(signature), vector.signature);
      const verifier = await ed25519PublicFromBytes(fromHex(vector.publicKey));
      assert.ok(await crypto.subtle.verify("Ed25519", verifier, signature, fromHex(vector.message)));
      signature[0] = (signature[0] ?? 0) ^ 0x01;
      assert.equal(await crypto.subtle.verify("Ed25519", verifier, signature, fromHex(vector.message)), false);
    });
  }
});
