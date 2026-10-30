import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { BUILT_IN_TYPES, recordFromTemplate } from "../../src/records/types.ts";

describe("built-in record templates", () => {
  it("keeps payment card fields without duplicating the type", () => {
    const cards = BUILT_IN_TYPES.filter((template) => template.id === "payment-card");
    assert.equal(cards.length, 1);
    assert.deepEqual(cards[0]?.sections[0]?.fields.map((field) => field.label), ["Cardholder", "Number", "Expires", "Security code", "PIN", "Card type", "Billing address"]);
  });

  it("gives an identity contact and social fields, and lets more be added", () => {
    const identity = BUILT_IN_TYPES.find((template) => template.id === "identity");
    assert.ok(identity);
    const labels = identity.sections[0]?.fields.map((field) => field.label);
    for (const label of ["First name", "Last name", "Date of birth", "Email", "Phone", "Address", "Job title", "Organization", "Website", "Social profiles"]) {
      assert.ok(labels?.includes(label), `missing ${label}`);
    }
    const document = recordFromTemplate(identity, "Partner");
    document.sections[0]?.fields.push({ label: "Preferred messenger", type: "text", value: null });
    assert.equal(document.sections[0]?.fields.at(-1)?.label, "Preferred messenger");
  });

  it("has a contact template for partners and external people", () => {
    const contact = BUILT_IN_TYPES.find((template) => template.id === "contact");
    assert.ok(contact);
    assert.deepEqual(contact.sections[0]?.fields.map((field) => field.label), ["Name", "Role", "Company", "Email", "Phone", "Website", "Social profiles", "Notes"]);
    const document = recordFromTemplate(contact, "Partner contact");
    assert.equal(document.type, "contact");
    assert.equal(document.sections[0]?.fields.length, 8);
  });
});
