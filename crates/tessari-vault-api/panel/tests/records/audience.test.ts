// Who a new record is for, chosen as it is made: the whole company, a department or a team. The company is a group
// the groups list leaves out, so it is added, first; a group with no key yet is offered and cannot be chosen.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Bytes } from "../../src/crypto/bytes.ts";
import { audienceChoices, companyAndGroups } from "../../src/records/audience.ts";
import { readGroupList } from "../../src/records/share-targets.ts";

const [COMPANY, PLATFORM, DATA, ENGINEERING, SALES] = ["a", "b", "c", "d", "e"].map((c) => c.repeat(32)) as [string, string, string, string, string];
const key = (seed: number): Bytes => new Uint8Array(32).fill(seed);
const GROUPS = readGroupList({
  groups: [
    { id: PLATFORM, kind: "team", name: "Platform" },
    { id: SALES, kind: "department", name: "Sales" },
    { id: DATA, kind: "team", name: "Data" },
    { id: ENGINEERING, kind: "department", name: "Engineering" },
  ],
});

describe("a new record's audience", () => {
  it("puts the company first, then departments, then teams, each by name", () => {
    assert.ok(GROUPS !== null);
    assert.deepEqual(
      companyAndGroups({ id: COMPANY, name: "Northwind" }, GROUPS).map((group) => `${group.kind} ${group.name}`),
      ["company Northwind", "department Engineering", "department Sales", "team Data", "team Platform"],
    );
  });

  it("offers every group, and one with no key yet only with the words for why", () => {
    assert.ok(GROUPS !== null);
    const keys = new Map([
      [COMPANY, { keyId: "11".repeat(16), public: key(1) }],
      [DATA, { keyId: "22".repeat(16), public: key(2) }],
    ]);
    const choices = audienceChoices({ id: COMPANY, name: "Northwind" }, GROUPS, keys);
    assert.deepEqual(
      choices.map((choice) => [choice.label, choice.detail, choice.recipient === null ? choice.missing : choice.recipient.recipient]),
      [
        ["Northwind", "Everyone in the company", { kind: "group", group: COMPANY }],
        ["Engineering", "Department", "has no key yet"],
        ["Sales", "Department", "has no key yet"],
        ["Data", "Team", { kind: "group", group: DATA }],
        ["Platform", "Team", "has no key yet"],
      ],
    );
  });
});
