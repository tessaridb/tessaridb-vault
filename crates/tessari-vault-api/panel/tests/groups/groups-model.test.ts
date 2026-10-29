// The groups page's model: the company with its departments and the teams under each, in reading order; the member
// keys still waiting for a group's key; and the words for what the server refused when a group is made.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { createFailure, groupTree, readGroupRows, readPending, readRotationDue } from "../../src/groups/groups-model.ts";

const [COMPANY, ENGINEERING, SALES, PLATFORM, DATA, DIRECT] = ["a", "b", "c", "d", "e", "f"].map((c) => c.repeat(32)) as [string, string, string, string, string, string];

describe("the groups page", () => {
  it("nests teams under their department, and teams of the company itself under it", () => {
    const rows = readGroupRows({
      groups: [
        { id: PLATFORM, kind: "team", name: "Platform", parent: ENGINEERING },
        { id: SALES, kind: "department", name: "Sales", parent: COMPANY },
        { id: DIRECT, kind: "team", name: "Office", parent: COMPANY },
        { id: DATA, kind: "team", name: "Data", parent: ENGINEERING },
        { id: ENGINEERING, kind: "department", name: "Engineering", parent: COMPANY },
      ],
    });
    assert.ok(rows !== null);
    assert.deepEqual(
      groupTree({ id: COMPANY, name: "Northwind" }, rows).map((row) => `${"  ".repeat(row.depth)}${row.name}`),
      ["Northwind", "  Engineering", "    Data", "    Platform", "  Sales", "  Office"],
    );
  });

  it("refuses a list carrying a kind it does not know", () => {
    assert.equal(readGroupRows({ groups: [{ id: SALES, kind: "squad", name: "Sales", parent: COMPANY }] }), null);
  });

  it("reads the waiting member keys, and refuses one that is not a key", () => {
    const key = Buffer.alloc(32, 7).toString("base64url");
    const pending = readPending({ pending: [{ account: DATA, key_id: "11".repeat(16), agreement_public: key }] });
    assert.ok(pending !== null);
    assert.deepEqual(pending.map((each) => [each.account, each.keyId.length, each.agreementPublic.length]), [[DATA, 16, 32]]);
    assert.equal(readPending({ pending: [{ account: DATA, key_id: "11", agreement_public: key }] }), null);
  });

  it("says why a group could not be made", () => {
    assert.match(createFailure("wrong_parent", "fallback"), /department sits in the company/);
    assert.equal(createFailure("other", "fallback"), "fallback");
  });

  it("reads whether a group's key is due for a new one, as after a member left", () => {
    assert.equal(readRotationDue({ key_id: "11".repeat(16), public: "x", wrap: null, rotation_due: true }), true);
    assert.equal(readRotationDue({ key_id: "11".repeat(16), public: "x", wrap: null, rotation_due: false }), false);
    assert.equal(readRotationDue({ key_id: "11".repeat(16) }), null);
  });
});
