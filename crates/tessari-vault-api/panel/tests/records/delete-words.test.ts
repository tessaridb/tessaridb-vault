// What the record page says when a delete is refused: who may delete, a record already gone, a record that changed while
// the dialog was open; anything else keeps the general words it was given.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { deleteFailure } from "../../src/records/delete-words.ts";

describe("a refused delete", () => {
  it("names who may delete, a record already gone and one that changed", () => {
    assert.match(deleteFailure(403, "forbidden", "general"), /author or a company admin/);
    assert.match(deleteFailure(404, "not_found", "general"), /already gone/);
    assert.match(deleteFailure(409, "stale_revision", "general"), /changed/);
  });

  it("keeps the general words for anything else", () => {
    assert.equal(deleteFailure(503, "unavailable", "general"), "general");
    assert.equal(deleteFailure(409, "stale_shares", "general"), "general");
  });
});
