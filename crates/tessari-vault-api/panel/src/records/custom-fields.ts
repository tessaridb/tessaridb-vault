// A record's own fields: beyond its type's, the author adds text, secret, SSH key and file fields, named as they like.
// They live in one section of their own, the record's last, so they are sealed with the record like every other field
// and need no new format; the edit form takes them out to manage them and puts them back before it saves.

import type { RecordDocument } from "./document.ts";
import type { Field, FileValue, SshKeyValue } from "./fields.ts";

export const CUSTOM_SECTION = "Custom fields";
const MAX_LABEL = 64;

/** A field the author added: a line of text, a secret shown as dots, an SSH key or a file. */
export type CustomField =
  | { readonly label: string; readonly type: "text" | "concealed"; readonly value: string | null }
  | { readonly label: string; readonly type: "ssh-key"; readonly value: SshKeyValue | null }
  | { readonly label: string; readonly type: "file"; readonly value: FileValue | null };

const isCustom = (field: Field): field is CustomField =>
  field.type === "text" || field.type === "concealed" || field.type === "ssh-key" || field.type === "file";

/** The document without its own fields, and those fields; a type's section that happens to share the name stays. */
export function splitCustom(document: RecordDocument): { readonly base: RecordDocument; readonly custom: readonly CustomField[] } {
  const last = document.sections.at(-1);
  if (last === undefined || last.name !== CUSTOM_SECTION || !last.fields.every(isCustom)) {
    return { base: document, custom: [] };
  }
  return { base: { ...document, sections: document.sections.slice(0, -1) }, custom: last.fields.filter(isCustom) };
}

/** `base` with `custom` as its last section, or `base` as it is when there are none. */
export function joinCustom(base: RecordDocument, custom: readonly CustomField[]): RecordDocument {
  return custom.length === 0 ? base : { ...base, sections: [...base.sections, { name: CUSTOM_SECTION, fields: [...custom] }] };
}

/** Why `label` can't name a new field beside `taken`, in words, or `null` when it can. */
export function customLabelProblem(label: string, taken: readonly string[]): string | null {
  const name = label.trim();
  if (name === "") {
    return "Give the field a name.";
  }
  if (name.length > MAX_LABEL) {
    return "Keep the name to 64 characters.";
  }
  const folded = name.toLocaleLowerCase();
  return taken.some((other) => other.trim().toLocaleLowerCase() === folded) ? "This record already has a field called that." : null;
}
