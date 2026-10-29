// Editing a record without a DOM: typed values land on their fields, an emptied field holds nothing, an untouched one
// keeps its value, and a value of the wrong shape is refused with words naming the field's format.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { RecordDocument } from "../../src/records/document.ts";
import { edited, fieldAt } from "../../src/records/editing.ts";

const DOCUMENT: RecordDocument = {
  v: 1,
  type: "login",
  title: "Mail admin",
  sections: [
    {
      name: "",
      fields: [
        { type: "text", label: "Username", value: "admin" },
        { type: "concealed", label: "Password", value: "old" },
        { type: "url", label: "Website", value: null },
      ],
    },
    { name: "Contact", fields: [{ type: "email", label: "Email", value: "ops@example.com" }] },
  ],
};

describe("editing a record", () => {
  it("puts typed values on their fields, empties a cleared one, and keeps the rest", () => {
    const values = new Map<string, unknown>([
      [fieldAt(0, 1), "new secret"],
      [fieldAt(0, 2), "https://mail.example.com"],
      [fieldAt(1, 0), ""],
    ]);
    const result = edited(DOCUMENT, "  Mail admin (prod) ", values);
    assert.ok(result.ok);
    if (result.ok) {
      assert.equal(result.document.title, "Mail admin (prod)");
      assert.deepEqual(result.document.sections[0]?.fields.map((field) => field.value), ["admin", "new secret", "https://mail.example.com"]);
      assert.equal(result.document.sections[1]?.fields[0]?.value, null);
      assert.equal(result.document.type, "login");
    }
  });

  it("refuses a value of the wrong shape, naming the field and its format, and an empty title", () => {
    const result = edited(DOCUMENT, " ", new Map([[fieldAt(1, 0), "no-at-sign"], [fieldAt(0, 2), "mail.example.com"]]));
    assert.equal(result.ok, false);
    if (!result.ok) {
      assert.deepEqual([...result.problems.keys()].sort(), ["0.2", "1.0", "title"]);
      assert.match(result.problems.get("1.0") ?? "", /name@example\.com/);
      assert.match(result.problems.get("0.2") ?? "", /https:\/\//);
      assert.match(result.problems.get("title") ?? "", /title/);
    }
  });

  it("does not change the document it was given", () => {
    edited(DOCUMENT, "Other", new Map([[fieldAt(0, 0), "root"]]));
    assert.equal(DOCUMENT.title, "Mail admin");
    assert.equal(DOCUMENT.sections[0]?.fields[0]?.value, "admin");
  });
});
