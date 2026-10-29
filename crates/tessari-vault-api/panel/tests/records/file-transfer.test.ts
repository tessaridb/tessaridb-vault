// Moving a sealed file to and from the server: the size the server is told before upload is the size sealing makes,
// and the parts cover the file once, in order, none above the server's part ceiling.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { FILE_CHUNK_BYTES, fileKeyId, importFileKey, newFileKey, sealFile, sealedSize } from "../../src/crypto/files.ts";
import { PART_BYTES, partsOf } from "../../src/records/file-transfer.ts";
import { noise } from "../crypto/support.ts";

describe("moving a sealed file", () => {
  it("tells the server the size sealing makes", async () => {
    const own = newFileKey();
    for (const length of [0, 1, FILE_CHUNK_BYTES - 1, FILE_CHUNK_BYTES, FILE_CHUNK_BYTES + 1, 3 * FILE_CHUNK_BYTES + 5]) {
      const sealed = await sealFile(await importFileKey(own.key), { company: "c", record: "r", file: "f", keyId: fileKeyId(own.keyId) }, noise(length));
      assert.equal(sealedSize(length), sealed.length, `${length} bytes`);
    }
  });

  it("sends and reads it in parts of at most 4 MiB that cover it once, in order", () => {
    assert.equal(PART_BYTES, 4 * 1024 * 1024);
    assert.deepEqual(partsOf(10), [{ offset: 0, length: 10 }]);
    assert.deepEqual(partsOf(PART_BYTES), [{ offset: 0, length: PART_BYTES }]);
    assert.deepEqual(partsOf(2 * PART_BYTES + 1), [
      { offset: 0, length: PART_BYTES },
      { offset: PART_BYTES, length: PART_BYTES },
      { offset: 2 * PART_BYTES, length: 1 },
    ]);
    assert.deepEqual(partsOf(0), []);
  });
});
