// HPKE Base mode (RFC 9180) for DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, AES-256-GCM, checked against the CFRG
// test vector for exactly that suite; then the vault's key wrap (algorithm 0x02) and what its `info` binds.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

import { random, type Bytes } from "../../src/crypto/bytes.ts";
import { encapsulateWith, keySchedule, open, sealWith } from "../../src/crypto/hpke.ts";
import { generatePair, publicBytes, x25519PrivateFromBytes } from "../../src/crypto/keys.ts";
import { unwrapSecret, wrapSecret, type WrapContext } from "../../src/crypto/wraps.ts";
import { flipped, fromHex, refusedWith, toHex } from "./support.ts";

interface Vector {
  info: string;
  skEm: string;
  pkEm: string;
  skRm: string;
  pkRm: string;
  enc: string;
  shared_secret: string;
  key: string;
  base_nonce: string;
  exporter_secret: string;
  encryptions: { aad: string; ct: string; nonce: string; pt: string }[];
}

const FILE = new URL("../vectors/hpke-base-x25519-sha256-aes256gcm.json", import.meta.url);
const VECTOR = (JSON.parse(readFileSync(FILE, "utf8")) as { vector: Vector }).vector;
const FIRST = VECTOR.encryptions[0];

async function ephemeral(): Promise<{ privateKey: CryptoKey; publicBytes: Uint8Array<ArrayBuffer> }> {
  return { privateKey: await x25519PrivateFromBytes(fromHex(VECTOR.skEm), false), publicBytes: fromHex(VECTOR.pkEm) };
}

describe("HPKE Base (RFC 9180) against the CFRG vector", () => {
  it("encapsulates to the vector's shared secret and encapsulated key", async () => {
    const { sharedSecret, enc } = await encapsulateWith(await ephemeral(), fromHex(VECTOR.pkRm));
    assert.equal(toHex(enc), VECTOR.enc);
    assert.equal(toHex(sharedSecret), VECTOR.shared_secret);
  });

  it("derives the vector's key, base nonce and exporter secret", async () => {
    const schedule = await keySchedule(fromHex(VECTOR.shared_secret), fromHex(VECTOR.info));
    assert.equal(toHex(schedule.key), VECTOR.key);
    assert.equal(toHex(schedule.baseNonce), VECTOR.base_nonce);
    assert.equal(toHex(schedule.exporterSecret), VECTOR.exporter_secret);
  });

  it("seals the first plaintext to the vector's ciphertext", async () => {
    assert.ok(FIRST);
    assert.equal(FIRST.nonce, VECTOR.base_nonce, "the first encryption uses the base nonce");
    const sealed = await sealWith(
      await ephemeral(),
      fromHex(VECTOR.pkRm),
      fromHex(VECTOR.info),
      fromHex(FIRST.aad),
      fromHex(FIRST.pt),
    );
    assert.equal(toHex(sealed.enc), VECTOR.enc);
    assert.equal(toHex(sealed.ciphertext), FIRST.ct);
  });

  it("opens the vector's ciphertext with the recipient's private key", async () => {
    assert.ok(FIRST);
    const recipient = await x25519PrivateFromBytes(fromHex(VECTOR.skRm), false);
    const plaintext = await open(
      recipient,
      fromHex(VECTOR.pkRm),
      fromHex(VECTOR.enc),
      fromHex(VECTOR.info),
      fromHex(FIRST.aad),
      fromHex(FIRST.ct),
    );
    assert.equal(toHex(plaintext), FIRST.pt);
  });

  it("refuses the ciphertext under a different info, aad or encapsulated key", async () => {
    assert.ok(FIRST);
    const recipient = await x25519PrivateFromBytes(fromHex(VECTOR.skRm), false);
    const args = [fromHex(VECTOR.enc), fromHex(VECTOR.info), fromHex(FIRST.aad), fromHex(FIRST.ct)] as const;
    for (const position of [0, 1, 2, 3] as const) {
      const changed: [Bytes, Bytes, Bytes, Bytes] = [...args];
      changed[position] = flipped(args[position], 0);
      await refusedWith(open(recipient, fromHex(VECTOR.pkRm), ...changed), "unauthentic");
    }
  });
});

async function recipient(): Promise<{ privateKey: CryptoKey; publicBytes: Uint8Array<ArrayBuffer> }> {
  const pair = await generatePair("X25519");
  return { privateKey: pair.privateKey, publicBytes: await publicBytes(pair.publicKey) };
}

function context(): WrapContext {
  return {
    company: "company-1",
    wrappedKind: "record",
    wrappedKeyId: random(16),
    recipientKind: "member",
    recipientKeyId: random(16),
  };
}

describe("key wrap (algorithm 0x02)", () => {
  it("unwraps the secret it wrapped for the recipient", async () => {
    const member = await recipient();
    const secret = random(32);
    const ctx = context();
    const wrap = await wrapSecret(member.publicBytes, ctx, secret);
    assert.equal(wrap[0], 1);
    assert.equal(wrap[1], 0x02);
    assert.equal(toHex(await unwrapSecret(member.privateKey, member.publicBytes, ctx, wrap)), toHex(secret));
  });

  it("refuses a wrap read under any other company, key kind, key id or recipient", async () => {
    const member = await recipient();
    const ctx = context();
    const wrap = await wrapSecret(member.publicBytes, ctx, random(32));
    const others: WrapContext[] = [
      { ...ctx, company: "company-2" },
      { ...ctx, wrappedKind: "group" },
      { ...ctx, wrappedKeyId: random(16) },
      { ...ctx, recipientKind: "group" },
      { ...ctx, recipientKeyId: random(16) },
    ];
    for (const other of others) {
      await refusedWith(unwrapSecret(member.privateKey, member.publicBytes, other, wrap), "unauthentic");
    }
  });

  it("refuses a wrap for another recipient's key", async () => {
    const member = await recipient();
    const stranger = await recipient();
    const ctx = context();
    const wrap = await wrapSecret(member.publicBytes, ctx, random(32));
    await refusedWith(unwrapSecret(stranger.privateKey, stranger.publicBytes, ctx, wrap), "unauthentic");
  });

  it("refuses an unknown format version or algorithm before any key agreement", async () => {
    const member = await recipient();
    const ctx = context();
    const wrap = await wrapSecret(member.publicBytes, ctx, random(32));
    const version = wrap.slice();
    version[0] = 2;
    await refusedWith(unwrapSecret(member.privateKey, member.publicBytes, ctx, version), "unknown-version");
    const algorithm = wrap.slice();
    algorithm[1] = 0x01;
    await refusedWith(unwrapSecret(member.privateKey, member.publicBytes, ctx, algorithm), "unknown-algorithm");
    await refusedWith(unwrapSecret(member.privateKey, member.publicBytes, ctx, wrap.slice(0, 20)), "malformed");
  });

  it("encapsulates afresh for every wrap of the same secret", async () => {
    const member = await recipient();
    const ctx = context();
    const secret = random(32);
    const first = await wrapSecret(member.publicBytes, ctx, secret);
    const second = await wrapSecret(member.publicBytes, ctx, secret);
    assert.notEqual(toHex(first), toHex(second));
  });
});
