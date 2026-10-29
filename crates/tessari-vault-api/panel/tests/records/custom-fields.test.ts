// A record's own fields: the author adds a text or a secret field under a name of their choosing; they are kept in one
// section of their own, a name is required and used once, and taking them out and putting them back changes nothing.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { CUSTOM_SECTION, customLabelProblem, joinCustom, splitCustom } from "../../src/records/custom-fields.ts";
import type { RecordDocument } from "../../src/records/document.ts";

const BASE: RecordDocument = {
  v: 1,
  type: "login",
  title: "Mail admin",
  sections: [{ name: "", fields: [{ type: "text", label: "Username", value: "admin" }] }],
};

describe("custom fields", () => {
  it("adds the author's fields in a section of their own, after the type's", () => {
    const joined = joinCustom(BASE, [
      { label: "Account number", type: "text", value: "42" },
      { label: "Recovery code", type: "concealed", value: null },
    ]);
    assert.deepEqual(joined.sections, [
      BASE.sections[0],
      {
        name: CUSTOM_SECTION,
        fields: [
          { label: "Account number", type: "text", value: "42" },
          { label: "Recovery code", type: "concealed", value: null },
        ],
      },
    ]);
  });

  it("adds no section when there are none", () => {
    assert.deepEqual(joinCustom(BASE, []), BASE);
  });

  it("takes them back out as they were put in", () => {
    const custom = [{ label: "PIN", type: "concealed", value: "1234" }] as const;
    const split = splitCustom(joinCustom(BASE, custom));
    assert.deepEqual(split.base, BASE);
    assert.deepEqual(split.custom, custom);
  });

  it("leaves a type's own section of the same name with the type", () => {
    const typed: RecordDocument = { ...BASE, sections: [...BASE.sections, { name: CUSTOM_SECTION, fields: [{ type: "url", label: "Site", value: null }] }] };
    const split = splitCustom(typed);
    assert.deepEqual(split.base, typed);
    assert.deepEqual(split.custom, []);
  });

  it("asks for a name, used once and not too long", () => {
    assert.equal(customLabelProblem("  ", ["Username"]), "Give the field a name.");
    assert.equal(customLabelProblem("username", ["Username"]), "This record already has a field called that.");
    assert.equal(customLabelProblem("x".repeat(65), []), "Keep the name to 64 characters.");
    assert.equal(customLabelProblem("Account number", ["Username"]), null);
  });
});
