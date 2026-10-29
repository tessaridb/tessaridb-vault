// Whom a record can be shared with from its page: the company's active members and its groups, without its author or
// anyone it is shared with already; each carries the public key the share wraps to, or says why it has none.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Bytes } from "../../src/crypto/bytes.ts";
import { filterTargets, readGroupList, readMemberList, sharedRows, shareTargets } from "../../src/records/share-targets.ts";
import type { DirectoryKey } from "../../src/records/shares-model.ts";

const [AUTHOR, ADMIN, LEFT, NEW, TEAM, DEPT] = ["a", "b", "c", "e", "d", "f"].map((c) => c.repeat(32)) as [string, string, string, string, string, string];
const key = (seed: number): Bytes => new Uint8Array(32).fill(seed);

const MEMBERS = readMemberList({
  members: [
    { account: { id: AUTHOR, email: "author@x.test", name: "Ann Author" }, role: "member", state: "active" },
    { account: { id: ADMIN, email: "admin@x.test", name: "Bo Admin" }, role: "admin", state: "active" },
    { account: { id: LEFT, email: "left@x.test", name: "Cy Left" }, role: "member", state: "suspended" },
    { account: { id: NEW, email: "new@x.test", name: "Ed New" }, role: "member", state: "active" },
  ],
});
const GROUPS = readGroupList({
  groups: [
    { id: TEAM, kind: "team", name: "Platform", parent: DEPT },
    { id: DEPT, kind: "department", name: "Engineering", parent: "" },
  ],
});
const DIRECTORY = new Map<string, DirectoryKey>([[ADMIN, { keyId: "55".repeat(16), agreementPublic: key(2), signingPublic: key(3) }]]);

describe("share targets", () => {
  it("lists active members and groups but the author and the shared, each with its key or why it has none", () => {
    assert.ok(MEMBERS !== null && GROUPS !== null);
    const targets = shareTargets(MEMBERS, GROUPS, DIRECTORY, new Map([[TEAM, { keyId: "66".repeat(16), public: key(4) }]]), AUTHOR, [DEPT]);
    assert.deepEqual(
      targets.map((target) => [target.label, target.recipient === null ? target.missing : target.recipient.recipient.kind]),
      [
        ["Bo Admin", "member"],
        ["Ed New", "hasn't set an unlock password yet"],
        ["Platform", "group"],
      ],
    );
  });

  it("keeps the targets whose name or detail holds the filter, ignoring case", () => {
    assert.ok(MEMBERS !== null && GROUPS !== null);
    const targets = shareTargets(MEMBERS, GROUPS, DIRECTORY, new Map(), AUTHOR, []);
    assert.deepEqual(filterTargets(targets, "  eD ").map((target) => target.label), ["Ed New"]);
    assert.deepEqual(filterTargets(targets, "DEPARTMENT").map((target) => target.label), ["Engineering"]);
    assert.equal(filterTargets(targets, "").length, targets.length);
  });

  it("names every share the record has, with what it lets them do", () => {
    assert.ok(MEMBERS !== null && GROUPS !== null);
    const rows = sharedRows(
      [
        { kind: "member", target: LEFT, right: "read", keyId: "77".repeat(16) },
        { kind: "group", target: TEAM, right: "edit", keyId: "66".repeat(16) },
        { kind: "member", target: "9".repeat(32), right: "read", keyId: "88".repeat(16) },
      ],
      MEMBERS,
      GROUPS,
    );
    assert.deepEqual(
      rows.map((row) => [row.target, row.label, row.detail, row.right]),
      [
        [LEFT, "Cy Left", "left@x.test", "read"],
        [TEAM, "Platform", "Team", "edit"],
        ["9".repeat(32), "A member no longer listed", "", "read"],
      ],
    );
  });

  it("refuses an answer it cannot read", () => {
    assert.equal(readMemberList({ members: [{ account: { id: "x" }, role: "member", state: "active" }] }), null);
    assert.equal(readGroupList({ groups: [{ id: TEAM, kind: "tribe", name: "x", parent: "" }] }), null);
  });
});
