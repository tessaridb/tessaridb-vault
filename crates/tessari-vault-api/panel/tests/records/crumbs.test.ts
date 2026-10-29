// The breadcrumb trail of the record pages: where the page sits under Records, every step a link back except the page
// itself, so the edit page returns to its record and a chosen type returns to the choice.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { editCrumbs, newCrumbs, recordCrumbs } from "../../src/records/crumbs.ts";

describe("the record pages' trail", () => {
  it("puts a record under Records, the record itself not a link", () => {
    assert.deepEqual(recordCrumbs("#/", "Mail admin"), [
      { label: "Records", href: "#/" },
      { label: "Mail admin", href: null },
    ]);
  });

  it("puts the edit page under its record, which links back to it", () => {
    assert.deepEqual(editCrumbs("#/", "#/records/abc", "Mail admin"), [
      { label: "Records", href: "#/" },
      { label: "Mail admin", href: "#/records/abc" },
      { label: "Edit", href: null },
    ]);
  });

  it("names a record whose title is not known yet, or is empty", () => {
    assert.equal(editCrumbs("#/", "#/records/abc", null)[1]?.label, "Record");
    assert.equal(recordCrumbs("#/", "")[1]?.label, "Untitled");
  });

  it("puts a new record under Records, and a chosen type under the choice", () => {
    assert.deepEqual(newCrumbs("#/", "#/records/new", null), [
      { label: "Records", href: "#/" },
      { label: "New record", href: null },
    ]);
    assert.deepEqual(newCrumbs("#/", "#/records/new", "Login"), [
      { label: "Records", href: "#/" },
      { label: "New record", href: "#/records/new" },
      { label: "Login", href: null },
    ]);
  });
});
