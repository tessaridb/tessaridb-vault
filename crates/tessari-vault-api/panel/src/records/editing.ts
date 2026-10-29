// Editing a record without a DOM: the values typed into the form put on their fields, each checked by the same reader
// that checks a decrypted record, so the page never seals a value it would refuse to open. An emptied field holds
// nothing; a field the form did not touch keeps its value.

import { VaultCryptoError } from "../crypto/errors.ts";
import type { RecordDocument } from "./document.ts";
import { readField, type Field, type FieldType } from "./fields.ts";

/** Where a field sits: its section's index and its own, as `section.field`. */
export type FieldAt = string;
export const fieldAt = (section: number, field: number): FieldAt => `${section}.${field}`;

const FORMAT: Partial<Record<FieldType, string>> = {
  url: "Use a full address starting with https://.",
  email: "Use an address like name@example.com.",
  phone: "Use digits, spaces and + ( ) - only.",
  number: "Use digits only.",
  date: "Use a date like 2027-02-28.",
  "month-year": "Use a month like 2028-11.",
  otp: "Paste the setup key exactly as the issuer showed it.",
};

function problem(type: FieldType, refused: unknown): string {
  if (refused instanceof VaultCryptoError && refused.reason === "too-large") {
    return "This value is too long to keep.";
  }
  return FORMAT[type] ?? "This value isn't in a form the record can keep.";
}

/** The document with `title` and `values` in place, or the fields (and `title`) whose value was refused, in words. */
export function edited(
  document: RecordDocument,
  title: string,
  values: ReadonlyMap<FieldAt, unknown>,
): { readonly ok: true; readonly document: RecordDocument } | { readonly ok: false; readonly problems: ReadonlyMap<string, string> } {
  const problems = new Map<string, string>();
  const named = title.trim();
  if (named === "") {
    problems.set("title", "Give the record a title, so you can find it again.");
  }
  const sections = document.sections.map((section, sectionIndex) => ({
    name: section.name,
    fields: section.fields.map((field, fieldIndex): Field => {
      const at = fieldAt(sectionIndex, fieldIndex);
      const typed = values.has(at) ? values.get(at) : field.value;
      try {
        return readField({ label: field.label, type: field.type, value: typed === "" ? null : typed });
      } catch (refused) {
        problems.set(at, problem(field.type, refused));
        return field;
      }
    }),
  }));
  return problems.size === 0 ? { ok: true, document: { ...document, title: named, sections } } : { ok: false, problems };
}
