// A record revision's body (algorithm 0x01): sealed under a content key derived from the record key with a fresh salt,
// bound to its company, record, revision and key id, and refused under any context other than its own.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { hex, random, utf8, type Bytes } from "../../src/crypto/bytes.ts";
import {
  importRecordKey,
  openRevision,
  sealRevision,
  type RecordContext,
} from "../../src/crypto/record.ts";
import { flipped, noise, refusedWith, toHex } from "./support.ts";

const PLAINTEXT = utf8('{"title":"Mail server","password":"correct horse"}');

async function fixture(): Promise<{
  key: CryptoKey;
  keys: Map<string, CryptoKey>;
  context: RecordContext;
}> {
  const keyId = random(16);
  const key = await importRecordKey(random(32));
  return {
    key,
    keys: new Map([[hex(keyId), key]]),
    context: { company: "company-1", record: "record-1", revision: 1, keyId },
  };
}

// Header offsets: [version 1][alg 1][key id 16][salt 32][nonce 12][ciphertext ‖ tag].
const SALT_AT = 18;
const NONCE_AT = 50;
const BODY_AT = 62;

describe("record body (algorithm 0x01)", () => {
  it("opens what it sealed, with the header the format names", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, PLAINTEXT);
    assert.equal(body[0], 1);
    assert.equal(body[1], 0x01);
    assert.equal(toHex(body.slice(2, SALT_AT)), toHex(context.keyId));
    assert.equal(body.length, BODY_AT + PLAINTEXT.length + 16);
    assert.equal(
      toHex(await openRevision(keys, context, body)),
      toHex(PLAINTEXT),
    );
  });

  it("opens an empty revision it sealed", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, new Uint8Array(0));
    assert.equal((await openRevision(keys, context, body)).length, 0);
  });

  it("opens every length it sealed: the block edges, a large body and 50 lengths drawn from a fixed seed", async () => {
    const { key, keys, context } = await fixture();
    let seed = 0x2f6b_3c1d;
    const drawn = Array.from({ length: 50 }, () => {
      seed = (Math.imul(seed, 1_103_515_245) + 12_345) >>> 0;
      return seed % 200_000;
    });
    for (const length of [0, 1, 15, 16, 17, 1023, 65_536, 65_537, ...drawn]) {
      const plaintext = noise(length);
      const body = await sealRevision(key, context, plaintext);
      assert.equal(body.length, BODY_AT + length + 16, `length ${length}`);
      // One boolean, so a failure names the length instead of printing two bodies of up to 400 000 hex digits.
      assert.ok(
        toHex(await openRevision(keys, context, body)) === toHex(plaintext),
        `length ${length}`,
      );
    }
  });

  it("draws a fresh salt and nonce for every revision", async () => {
    const { key, context } = await fixture();
    const first = await sealRevision(key, context, PLAINTEXT);
    const second = await sealRevision(key, context, PLAINTEXT);
    assert.notEqual(
      toHex(first.slice(SALT_AT, NONCE_AT)),
      toHex(second.slice(SALT_AT, NONCE_AT)),
    );
    assert.notEqual(
      toHex(first.slice(NONCE_AT, BODY_AT)),
      toHex(second.slice(NONCE_AT, BODY_AT)),
    );
  });

  it("refuses a body moved to another company, record or revision", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, PLAINTEXT);
    for (const moved of [
      { ...context, company: "company-2" },
      { ...context, record: "record-2" },
      { ...context, revision: 2 },
    ]) {
      await refusedWith(openRevision(keys, moved, body), "unauthentic");
    }
  });

  it("refuses a header whose key id disagrees with the record's own", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, PLAINTEXT);
    const otherId = random(16);
    keys.set(hex(otherId), key);
    await refusedWith(
      openRevision(keys, { ...context, keyId: otherId }, body),
      "key-mismatch",
    );
  });

  it("refuses a key id it does not hold without trying the keys it does hold", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, PLAINTEXT);
    // The header names the held key; the record's own row names one that is not held.
    const unheld = random(16);
    const forged = body.slice();
    forged.set(unheld, 2);
    await refusedWith(
      openRevision(keys, { ...context, keyId: unheld }, forged),
      "unknown-key",
    );
  });

  it("refuses a body whose salt, nonce, ciphertext or tag was changed", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, PLAINTEXT);
    for (const index of [SALT_AT, NONCE_AT, BODY_AT, body.length - 1]) {
      await refusedWith(
        openRevision(keys, context, flipped(body, index)),
        "unauthentic",
      );
    }
  });

  it("refuses an unknown version, an unknown algorithm and a truncated body", async () => {
    const { key, keys, context } = await fixture();
    const body = await sealRevision(key, context, PLAINTEXT);
    const version: Bytes = body.slice();
    version[0] = 2;
    await refusedWith(openRevision(keys, context, version), "unknown-version");
    const algorithm: Bytes = body.slice();
    algorithm[1] = 0x05;
    await refusedWith(
      openRevision(keys, context, algorithm),
      "unknown-algorithm",
    );
    await refusedWith(
      openRevision(keys, context, body.slice(0, BODY_AT)),
      "malformed",
    );
  });
});
