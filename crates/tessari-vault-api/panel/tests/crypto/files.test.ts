// Files (algorithm 0x05): chunked AES-256-GCM in the STREAM construction, so a reordered, truncated, extended or
// spliced file is refused as a whole, with a stated ceiling of 256 MiB per file.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { concat, hex, random, type Bytes } from "../../src/crypto/bytes.ts";
import { FILE_CHUNK_BYTES, FILE_MAX_BYTES, openFile, sealFile, type FileContext } from "../../src/crypto/files.ts";
import { importRecordKey } from "../../src/crypto/record.ts";
import { flipped, noise, refusedWith, toHex } from "./support.ts";

// Header: [version 1][alg 1][key id 16][salt 32][prefix 7]; each chunk is its plaintext plus a 16-byte tag.
const HEADER_BYTES = 57;
const SEALED_CHUNK = FILE_CHUNK_BYTES + 16;

async function fixture(): Promise<{ key: CryptoKey; keys: Map<string, CryptoKey>; context: FileContext }> {
  const keyId = random(16);
  const key = await importRecordKey(random(32));
  return {
    key,
    keys: new Map([[hex(keyId), key]]),
    context: { company: "company-1", record: "record-1", file: "file-1", keyId },
  };
}

function chunksOf(sealed: Bytes): Bytes[] {
  const chunks: Bytes[] = [];
  for (let at = HEADER_BYTES; at < sealed.length; at += SEALED_CHUNK) {
    chunks.push(sealed.slice(at, at + SEALED_CHUNK));
  }
  return chunks;
}

function rebuilt(sealed: Bytes, chunks: Bytes[]): Bytes {
  return concat(sealed.slice(0, HEADER_BYTES), ...chunks);
}

describe("files (algorithm 0x05, STREAM)", () => {
  for (const size of [0, 1, FILE_CHUNK_BYTES, FILE_CHUNK_BYTES + 1, 3 * FILE_CHUNK_BYTES + 5]) {
    it(`opens a ${size}-byte file it sealed, in ${Math.max(1, Math.ceil(size / FILE_CHUNK_BYTES))} chunk(s)`, async () => {
      const { key, keys, context } = await fixture();
      const data = noise(size);
      const sealed = await sealFile(key, context, data);
      assert.equal(sealed[0], 1);
      assert.equal(sealed[1], 0x05);
      const chunks = Math.max(1, Math.ceil(size / FILE_CHUNK_BYTES));
      assert.equal(sealed.length, HEADER_BYTES + size + 16 * chunks);
      assert.equal(toHex(await openFile(keys, context, sealed)), toHex(data));
    });
  }

  it("refuses a file cut short at a chunk boundary", async () => {
    const { key, keys, context } = await fixture();
    const sealed = await sealFile(key, context, noise(2 * FILE_CHUNK_BYTES + 9));
    await refusedWith(openFile(keys, context, rebuilt(sealed, chunksOf(sealed).slice(0, 2))), "unauthentic");
  });

  it("refuses a file with a chunk added after its last", async () => {
    const { key, keys, context } = await fixture();
    const sealed = await sealFile(key, context, noise(2 * FILE_CHUNK_BYTES));
    const [first, second] = chunksOf(sealed) as [Bytes, Bytes];
    await refusedWith(openFile(keys, context, rebuilt(sealed, [first, second, first])), "unauthentic");
  });

  it("refuses a file whose chunks were reordered", async () => {
    const { key, keys, context } = await fixture();
    const sealed = await sealFile(key, context, noise(3 * FILE_CHUNK_BYTES));
    const [first, second, third] = chunksOf(sealed) as [Bytes, Bytes, Bytes];
    await refusedWith(openFile(keys, context, rebuilt(sealed, [second, first, third])), "unauthentic");
  });

  it("refuses a chunk taken from another file under the same record key", async () => {
    const { key, keys, context } = await fixture();
    const mine = await sealFile(key, context, noise(2 * FILE_CHUNK_BYTES));
    const other = await sealFile(key, { ...context, file: "file-2" }, noise(2 * FILE_CHUNK_BYTES));
    const [ownFirst] = chunksOf(mine) as [Bytes];
    const [, otherSecond] = chunksOf(other) as [Bytes, Bytes];
    await refusedWith(openFile(keys, context, rebuilt(mine, [ownFirst, otherSecond])), "unauthentic");
    await refusedWith(openFile(keys, { ...context, file: "file-2" }, mine), "unauthentic");
  });

  it("refuses a changed header, a header naming another key and a key it does not hold", async () => {
    const { key, keys, context } = await fixture();
    const sealed = await sealFile(key, context, noise(100));
    await refusedWith(openFile(keys, context, flipped(sealed, HEADER_BYTES - 1)), "unauthentic");
    const otherId = random(16);
    keys.set(hex(otherId), key);
    await refusedWith(openFile(keys, { ...context, keyId: otherId }, sealed), "key-mismatch");
    await refusedWith(openFile(new Map(), context, sealed), "unknown-key");
  });

  it("refuses an unknown version or algorithm and a body shorter than one tag", async () => {
    const { key, keys, context } = await fixture();
    const sealed = await sealFile(key, context, noise(10));
    const version = sealed.slice();
    version[0] = 2;
    await refusedWith(openFile(keys, context, version), "unknown-version");
    const algorithm = sealed.slice();
    algorithm[1] = 0x01;
    await refusedWith(openFile(keys, context, algorithm), "unknown-algorithm");
    await refusedWith(openFile(keys, context, sealed.slice(0, HEADER_BYTES + 15)), "malformed");
  });

  it("refuses to seal a file above the ceiling, before reading it", async () => {
    const { key, context } = await fixture();
    // A zero-filled buffer the size check rejects before any byte of it is read or encrypted.
    await refusedWith(sealFile(key, context, new Uint8Array(FILE_MAX_BYTES + 1)), "too-large");
  });
});
