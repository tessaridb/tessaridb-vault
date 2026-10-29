// The records views' readers: a row, an opened record and a page of them, each refused unless every part has its shape.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { encode } from "../../src/base64url.ts";
import { openedPagePath, readOpened, readOpenedPage, readRecipient, readRow } from "../../src/records-model.ts";

const ID = (digit: string) => digit.repeat(32);
const row = {
  id: ID("1"),
  record_type: "login",
  revision: 2,
  key_id: ID("2"),
  author: ID("3"),
  created: 1_761_700_000,
  updated: 1_761_700_100,
  rotation_due: false,
};
const opened = {
  record: row,
  body: encode(Uint8Array.of(1, 2, 3)),
  signature: encode(Uint8Array.of(4, 5)),
  author_key: ID("4"),
  recipient: `member:${ID("5")}`,
  wrap: encode(Uint8Array.of(6)),
};

describe("the records readers", () => {
  it("read a row and refuse a malformed one", () => {
    assert.equal(readRow(row)?.recordType, "login");
    assert.equal(readRow({ ...row, id: "nope" }), null);
    assert.equal(readRow({ ...row, revision: 0 }), null);
    assert.equal(readRow({ ...row, rotation_due: "no" }), null);
  });

  it("read whom a wrap is kept under, member or group and nothing else", () => {
    assert.deepEqual(readRecipient(`group:${ID("a")}`), { kind: "group", keyId: ID("a") });
    assert.equal(readRecipient(`owner:${ID("a")}`), null);
    assert.equal(readRecipient(`member:${ID("a")}:x`), null);
  });

  it("read an opened record with its sealed parts as bytes", () => {
    const read = readOpened(opened);
    assert.ok(read !== null);
    assert.deepEqual(read.body, Uint8Array.of(1, 2, 3));
    assert.deepEqual(read.recipient, { kind: "member", keyId: ID("5") });
    assert.equal(readOpened({ ...opened, wrap: "not base64url!" }), null);
    assert.equal(readOpened({ ...opened, record: { ...row, author: "x" } }), null);
  });

  it("read a page only when every record in it reads", () => {
    assert.equal(readOpenedPage({ records: [opened, opened], next: ID("9") })?.records.length, 2);
    assert.equal(readOpenedPage({ records: [opened], next: null })?.next, null);
    assert.equal(readOpenedPage({ records: [opened, { ...opened, body: 7 }], next: null }), null);
    assert.equal(readOpenedPage({ records: [], next: "abc" }), null);
  });

  it("builds the page route with the cursor encoded", () => {
    assert.equal(openedPagePath("c1", null), "/api/companies/c1/records?opened=true");
    assert.equal(openedPagePath("c1", ID("f")), `/api/companies/c1/records?opened=true&after=${ID("f")}`);
  });
});
