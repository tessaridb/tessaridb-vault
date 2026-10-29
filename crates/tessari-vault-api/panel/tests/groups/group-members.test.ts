// Who is in a department or team, and whom an admin can add: the company's active members not in it already.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { addable, readGroupMembers } from "../../src/groups/group-members.ts";
import { readMemberList } from "../../src/records/share-targets.ts";

const [ANN, BO, CY, ED] = ["a", "b", "c", "e"].map((c) => c.repeat(32)) as [string, string, string, string];

describe("a group's members", () => {
  it("reads who is in the group, and refuses a malformed answer", () => {
    const members = readGroupMembers({ members: [{ id: ANN, email: "ann@x.test", name: "Ann" }] });
    assert.deepEqual(members, [{ account: ANN, email: "ann@x.test", name: "Ann" }]);
    assert.equal(readGroupMembers({ members: [{ id: "ann", email: "ann@x.test", name: "Ann" }] }), null);
  });

  it("offers the company's active members who are not in the group yet, by name", () => {
    const company = readMemberList({
      members: [
        { account: { id: ED, email: "ed@x.test", name: "Ed" }, role: "member", state: "active" },
        { account: { id: ANN, email: "ann@x.test", name: "Ann" }, role: "member", state: "active" },
        { account: { id: CY, email: "cy@x.test", name: "Cy" }, role: "member", state: "suspended" },
        { account: { id: BO, email: "bo@x.test", name: "Bo" }, role: "admin", state: "active" },
      ],
    });
    assert.ok(company !== null);
    assert.deepEqual(
      addable(company, [{ account: ANN, email: "ann@x.test", name: "Ann" }]).map((member) => member.name),
      ["Bo", "Ed"],
    );
  });
});
