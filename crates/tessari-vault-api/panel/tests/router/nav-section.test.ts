// Which sidebar section a page belongs to: a record, a group or a member's page marks its list, so the person sees
// where they are from any page under it.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { sectionOf } from "../../src/nav-section.ts";

describe("the sidebar section a page belongs to", () => {
  it("is its list for every page under it", () => {
    for (const name of ["records", "record", "newRecord", "editRecord"]) {
      assert.equal(sectionOf(name), "records", name);
    }
    assert.equal(sectionOf("groups"), "groups");
    assert.equal(sectionOf("group"), "groups");
    assert.equal(sectionOf("members"), "members");
    assert.equal(sectionOf("member"), "members");
    assert.equal(sectionOf("trail"), "trail");
    assert.equal(sectionOf("account"), "account");
  });

  it("is none for pages outside the sections", () => {
    for (const name of ["signIn", "unlock", "join", "", "constructor"]) {
      assert.equal(sectionOf(name), "", name);
    }
  });
});
