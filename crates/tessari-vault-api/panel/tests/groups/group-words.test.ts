// The groups page's buttons: each one's accessible name starts with the words it shows, and then names the group or
// person it acts on.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { giveButton, groupCrumbs, keyButton, takeOutButton } from "../../src/groups/group-words.ts";

describe("the groups page's buttons", () => {
  it("are named by the words they show, then by what they act on", () => {
    const buttons = [
      [keyButton("Engineering", false), "Engineering"],
      [keyButton("Engineering", true), "Engineering"],
      [giveButton("Engineering", 2), "Engineering"],
      [takeOutButton("Sam Member", "Engineering"), "Sam Member"],
    ] as const;
    for (const [words, subject] of buttons) {
      assert.ok(words.label.startsWith(words.text), `"${words.label}" does not start with "${words.text}"`);
      assert.ok(words.label.includes(subject), `"${words.label}" does not name ${subject}`);
    }
  });

  it("show what each does", () => {
    assert.equal(keyButton("Engineering", false).text, "Key this group");
    assert.equal(keyButton("Engineering", true).text, "Make a new key");
    assert.equal(giveButton("Engineering", 2).text, "Give access (2 waiting)");
    assert.equal(takeOutButton("Sam Member", "Engineering").text, "Take out");
  });

  it("trails a group's own page under Groups, the group itself unlinked", () => {
    assert.deepEqual(groupCrumbs("#/groups", "Engineering"), [
      { label: "Groups", href: "#/groups" },
      { label: "Engineering", href: null },
    ]);
    assert.deepEqual(groupCrumbs("#/groups", null), [
      { label: "Groups", href: "#/groups" },
      { label: "Group", href: null },
    ]);
  });
});
