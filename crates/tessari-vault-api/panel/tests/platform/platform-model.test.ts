// The platform admin's Companies page, without a DOM: the open deletions it reads, each company shown with the
// deletion waiting on it, the answer to creating one, and a refused step in words.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { platformFailure, platformRows, readCreated, readQueue } from "../../src/platform/platform-model.ts";
import type { Company } from "../../src/unlock-model.ts";

const company = (id: string, name: string, active: boolean, closing = false): Company => ({ id, name, active, closing, role: null });

describe("the open deletions", () => {
  it("reads a request waiting for confirmation and a confirmed one with its date", () => {
    assert.deepEqual(readQueue({ drops: [{ company: "c1", state: "requested" }, { company: "c2", state: "scheduled", drop_after: 1_793_904_800 }] }), [
      { company: "c1", standing: { state: "requested" } },
      { company: "c2", standing: { state: "scheduled", dropAfter: 1_793_904_800 } },
    ]);
    assert.equal(readQueue({ drops: [{ company: "c1", state: "scheduled" }] }), null);
    assert.equal(readQueue({ drops: "none" }), null);
  });
});

describe("the companies list", () => {
  it("shows each company's state and the deletion waiting on it", () => {
    const rows = platformRows(
      [company("c1", "Alpha", true), company("c2", "Beta", false), company("c3", "Gamma", false, true)],
      [{ company: "c1", standing: { state: "requested" } }, { company: "c3", standing: { state: "scheduled", dropAfter: 1 } }],
    );
    assert.deepEqual(
      rows.map((row) => [row.id, row.state, row.deletion?.state ?? null]),
      [
        ["c1", "active", "requested"],
        ["c2", "suspended", null],
        ["c3", "closing", "scheduled"],
      ],
    );
  });
});

describe("creating a company", () => {
  it("reads the owner's join token, which is answered once", () => {
    assert.deepEqual(readCreated({ id: "c9", name: "Delta", state: "active", role: null, owner_account_id: "a9", owner_enrolment: "t0k3n" }), { id: "c9", name: "Delta", token: "t0k3n" });
    assert.deepEqual(readCreated({ id: "c9", name: "Delta", state: "active", role: null, owner_account_id: "a9", owner_enrolment: null }), { id: "c9", name: "Delta", token: null });
    assert.equal(readCreated({ id: "c9" }), null);
  });
});

describe("a refused step", () => {
  it("says what to do next", () => {
    const refused = (code: string) => platformFailure({ ok: false, status: 400, code, message: "" });
    assert.match(refused("invalid"), /name/);
    assert.match(refused("forbidden"), /platform admin/);
    assert.match(refused("not_found"), /No owner/);
    assert.notEqual(refused("invalid"), refused("other"));
  });
});
