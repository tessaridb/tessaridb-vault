// Adding and managing members, without a DOM: the join link an admin hands on, the roles each role may give, and the
// words each refused change becomes.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { tokenFrom } from "../../src/join-model.ts";
import { joinLink, mayManage, memberCrumbs, memberFailure, removeWords, roleChoices } from "../../src/members/member-admin.ts";

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

describe("whom a person may manage", () => {
  it("is anyone for an owner, anyone but an owner for an admin, nobody for a member", () => {
    assert.equal(mayManage("owner", "owner"), true);
    assert.equal(mayManage("admin", "admin"), true);
    assert.equal(mayManage("admin", "member"), true);
    assert.equal(mayManage("admin", "owner"), false);
    assert.equal(mayManage("member", "member"), false);
  });
});

describe("a member's own page", () => {
  it("trails under Members, the person unlinked", () => {
    assert.deepEqual(memberCrumbs("#/members", "Kim Third"), [
      { label: "Members", href: "#/members" },
      { label: "Kim Third", href: null },
    ]);
    assert.deepEqual(memberCrumbs("#/members", null), [
      { label: "Members", href: "#/members" },
      { label: "Member", href: null },
    ]);
  });

  it("asks before removing someone, naming them and what changes", () => {
    const words = removeWords("Kim Third", "Northwind");
    assert.equal(words.title, "Remove \u201CKim Third\u201D from Northwind?");
    assert.match(words.body, /next request/);
    assert.match(words.body, /new keys/);
    assert.equal(words.confirm, "Remove Kim Third");
  });
});
