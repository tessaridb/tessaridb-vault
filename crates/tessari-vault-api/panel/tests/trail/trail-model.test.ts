// The company trail, without a DOM: the events and the check it reads, every action the server writes in words, the
// area each belongs to for the filter, who and what an event names, and when another page is there to read.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { actionWords, areaOf, pageFull, readEvents, readVerified, targetWords, verifyWords, whoWords } from "../../src/trail/trail-model.ts";

// Every action the server records, taken from the storage sources (grep of the quoted dotted names, 2026-10-29).
const ACTIONS = [
  "chain.append", "company.create", "company.resume", "company.suspend", "enrolment.issue", "file.complete", "file.create", "file.read",
  "group_key.keep", "group_key.read", "group_key.rotate", "group_key.wrap", "group.create", "group.delete", "group.member.add",
  "group.member.remove", "group.rename", "keys.escrow", "keys.keep", "keys.read", "keys.rewrap", "member.add", "member.change",
  "member.escrow.read", "member.recover", "member.recovery.redeem", "member.recovery.replay", "member.recovery.request", "member.remove",
  "record_type.define", "record.create", "record.delete", "record.read", "record.revise", "record.share", "record.unshare",
  "recovery.key.read", "recovery.key",
];

const event = { seq: 7, at: 1_761_700_000, actor: "a".repeat(32), action: "member.add", target: "b".repeat(32), reason: "" };

describe("what the trail reads", () => {
  it("takes the events and the check only in their shapes", () => {
    assert.deepEqual(readEvents({ events: [event] }), [event]);
    assert.equal(readEvents({ events: [{ ...event, seq: "7" }] }), null);
    assert.equal(readEvents({ items: [] }), null);
    assert.deepEqual(readVerified({ intact: true, broken_at: null }), { intact: true, brokenAt: null });
    assert.deepEqual(readVerified({ intact: false, broken_at: 12 }), { intact: false, brokenAt: 12 });
    assert.equal(readVerified({ intact: "yes" }), null);
  });
});

describe("every action in words", () => {
  it("says each action the server writes, in its own words", () => {
    const said = new Set<string>();
    for (const action of ACTIONS) {
      const words = actionWords(action);
      assert.notEqual(words, action, action);
      assert.match(words, /^[A-Z]/, action);
      said.add(words);
    }
    // Two actions saying the same words would make two different events read alike.
    assert.equal(said.size, ACTIONS.length);
  });

  it("keeps an action it does not know as written, rather than hiding the event", () => {
    assert.equal(actionWords("future.thing"), "future.thing");
  });

  it("files each action under one area for the filter", () => {
    assert.equal(areaOf("record.share"), "records");
    assert.equal(areaOf("file.read"), "records");
    assert.equal(areaOf("record_type.define"), "records");
    assert.equal(areaOf("member.recovery.request"), "members");
    assert.equal(areaOf("enrolment.issue"), "members");
    assert.equal(areaOf("group.member.add"), "groups");
    assert.equal(areaOf("group_key.rotate"), "groups");
    assert.equal(areaOf("keys.read"), "keys");
    assert.equal(areaOf("chain.append"), "keys");
    assert.equal(areaOf("recovery.key"), "keys");
    assert.equal(areaOf("company.suspend"), "company");
    for (const action of ACTIONS) {
      assert.notEqual(areaOf(action), "other", action);
    }
  });
});

describe("who and what an event names", () => {
  const names = new Map([["a".repeat(32), "Dana Owner"], ["b".repeat(32), "Kim Third"], ["c".repeat(32), "Engineering"]]);

  it("names a known person or group and shortens what it can't name", () => {
    assert.equal(whoWords("a".repeat(32), names), "Dana Owner");
    assert.equal(whoWords("d".repeat(32), names), "dddddddd");
    assert.equal(targetWords("b".repeat(32), names), "Kim Third");
    assert.equal(targetWords(`${"c".repeat(32)}:${"b".repeat(32)}`, names), "Kim Third in Engineering");
    assert.equal(targetWords("12", names), "12");
  });
});

describe("the trail's check and its pages", () => {
  it("says whether the chain holds and where it breaks", () => {
    assert.match(verifyWords({ intact: true, brokenAt: null }), /intact/);
    assert.match(verifyWords({ intact: false, brokenAt: 12 }), /Event 12/);
  });

  it("offers older events only when a page came back full", () => {
    assert.equal(pageFull(100), true);
    assert.equal(pageFull(99), false);
  });
});
