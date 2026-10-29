// Adding and managing members, without a DOM: the join link an admin hands on, the roles each role may give, and the
// words each refused change becomes.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { tokenFrom } from "../../src/join-model.ts";
import { joinLink, memberFailure, roleChoices } from "../../src/members/member-admin.ts";

const failure = (status: number, code: string) => ({ ok: false as const, status, code, message: "server words" });

describe("the join link an admin hands on", () => {
  it("opens this vault's join page with the token", () => {
    assert.equal(joinLink("https://vault.test", "/", "abc123"), "https://vault.test/#/join?token=abc123");
  });

  it("gives back the very token the join page reads, whatever it holds", () => {
    for (const token of ["abc123", "a&b=c", "100%/x?y", "c356a8c83f67bd80244a466a920a2402"]) {
      assert.equal(tokenFrom(joinLink("https://vault.test", "/", token)), token, token);
    }
    // Control: the link really carries the token escaped, so the round trip is not passing by leaving it out.
    assert.match(joinLink("https://vault.test", "/", "a&b"), /token=a%26b$/);
  });
});

describe("the roles a person may give", () => {
  it("is every role for an owner, member and admin for an admin, none for a member", () => {
    assert.deepEqual(roleChoices("owner"), ["member", "admin", "owner"]);
    assert.deepEqual(roleChoices("admin"), ["member", "admin"]);
    assert.deepEqual(roleChoices("member"), []);
  });
});

describe("what a refused change to a member says", () => {
  it("names each refusal the server gives", () => {
    assert.match(memberFailure(failure(409, "already_member")), /already in the company/);
    assert.match(memberFailure(failure(409, "last_owner")), /at least one owner/);
    assert.match(memberFailure(failure(409, "enrolled")), /already joined/);
    assert.match(memberFailure(failure(403, "forbidden")), /Only an owner/);
    assert.match(memberFailure(failure(404, "not_found")), /aren't in the company any more/);
    assert.match(memberFailure(failure(400, "invalid")), /name and a work email/);
    assert.match(memberFailure(failure(0, "network")), /didn't answer/);
  });
});
