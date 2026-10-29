// What the record page says when a share or its removal is refused: a target shared with already, one with no key yet,
// a key that moved, a record or its sharing that changed, who may share; anything else keeps the general words.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { removeShareFailure, sealFailure, shareFailure } from "../../src/records/share-words.ts";

describe("a refused share", () => {
  it("names each conflict the server can answer", () => {
    assert.match(shareFailure(409, "already_shared", "general"), /already/);
    assert.match(shareFailure(409, "no_key", "general"), /no key yet/);
    assert.match(shareFailure(409, "unknown_key", "general"), /key changed/);
    assert.match(shareFailure(409, "stale_revision", "general"), /Open it again/);
    assert.match(shareFailure(409, "stale_shares", "general"), /Open it again/);
    assert.match(shareFailure(403, "forbidden", "general"), /can read this record but not share it/);
  });

  it("keeps the general words for anything else", () => {
    assert.equal(shareFailure(503, "unavailable", "general"), "general");
    assert.equal(shareFailure(409, "rotation_due", "general"), "general");
  });
});

describe("a share this page would not seal", () => {
  it("says a key nobody vouched for apart from a record that would not open", () => {
    assert.match(sealFailure("unvouched-recipient"), /not vouched for/);
    assert.match(sealFailure("bad-signature"), /could not be opened to share/);
    assert.match(sealFailure("key-mismatch"), /could not be opened to share/);
  });
});

describe("a refused share removal", () => {
  it("names a share already gone and who may remove one", () => {
    assert.match(removeShareFailure(404, "not_found", "general"), /no longer shared/);
    assert.match(removeShareFailure(403, "forbidden", "general"), /can read this record but not change who it is shared with/);
    assert.equal(removeShareFailure(503, "unavailable", "general"), "general");
  });
});
