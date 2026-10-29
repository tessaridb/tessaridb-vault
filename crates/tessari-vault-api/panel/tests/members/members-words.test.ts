// What the members page says: each key state in words, and why vouching was refused, by the server or by this page.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { keyStateWords, vouchFailure, vouchRefused } from "../../src/members/members-words.ts";

describe("the members page's words", () => {
  it("says each key state", () => {
    assert.equal(keyStateWords("vouched"), "Key vouched for");
    assert.equal(keyStateWords("waiting"), "Key waiting to be vouched for");
    assert.equal(keyStateWords("no-key"), "Hasn't set an unlock password yet");
  });

  it("names who may vouch, a chain that moved, and keeps the general words otherwise", () => {
    assert.match(vouchFailure(403, "forbidden", "general"), /owner or an admin/);
    assert.match(vouchFailure(409, "stale_chain", "general"), /meanwhile/);
    assert.equal(vouchFailure(503, "unavailable", "general"), "general");
  });

  it("says why this page would not vouch", () => {
    assert.match(vouchRefused("unvouched-signer"), /owner or an admin the owner vouched for/);
    assert.match(vouchRefused("chain-broken"), /doesn't check out/);
  });
});
