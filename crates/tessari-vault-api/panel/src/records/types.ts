// The built-in record types: templates a new record starts from. A record may add any field of any type in any
// section; a company-defined type is a template of the same shape with its own id (served by the company, sealed to
// its members). The labels here are the defaults a new record is created with, and become part of its sealed content.

import type { RecordDocument } from "./document.ts";
import type { Field, FieldType } from "./fields.ts";

export interface TemplateField {
  label: string;
  type: FieldType;
}

export interface RecordTemplate {
  id: string;
  name: string;
  sections: { name: string; fields: TemplateField[] }[];
}

function main(name: string, id: string, fields: [string, FieldType][]): RecordTemplate {
  return { id, name, sections: [{ name: "", fields: fields.map(([label, type]) => ({ label, type })) }] };
}

export const BUILT_IN_TYPES: readonly RecordTemplate[] = [
  main("Login", "login", [
    ["Username", "text"],
    ["Password", "concealed"],
    ["Website", "url"],
    ["One-time password", "otp"],
  ]),
  main("Secure note", "secure-note", [["Note", "multiline"]]),
  main("SSH key", "ssh-key", [
    ["Key", "ssh-key"],
    ["Passphrase", "concealed"],
  ]),
  main("File", "file", [["File", "file"]]),
  main("Payment card", "payment-card", [
    ["Cardholder", "text"],
    ["Number", "concealed"],
    ["Expires", "month-year"],
    ["Security code", "concealed"],
    ["PIN", "concealed"],
  ]),
  main("Identity", "identity", [
    ["First name", "text"],
    ["Last name", "text"],
    ["Date of birth", "date"],
    ["Email", "email"],
    ["Phone", "phone"],
    ["Address", "multiline"],
  ]),
  main("Server", "server", [
    ["Address", "text"],
    ["Port", "number"],
    ["Username", "text"],
    ["Password", "concealed"],
  ]),
  main("Database", "database", [
    ["Kind", "text"],
    ["Host", "text"],
    ["Port", "number"],
    ["Database", "text"],
    ["Username", "text"],
    ["Password", "concealed"],
  ]),
  main("API credential", "api-credential", [
    ["Key id", "text"],
    ["Secret", "concealed"],
    ["Endpoint", "url"],
    ["Expires", "date"],
  ]),
  main("Wireless network", "wireless-network", [
    ["Network name", "text"],
    ["Password", "concealed"],
    ["Security", "text"],
  ]),
  main("Software license", "software-license", [
    ["Product", "text"],
    ["Version", "text"],
    ["License key", "concealed"],
    ["Licensed to", "text"],
    ["Email", "email"],
    ["Purchased", "date"],
    ["Expires", "date"],
  ]),
];

/** A new record of `template`'s type: its sections and labels, every value empty. */
export function recordFromTemplate(template: RecordTemplate, title: string): RecordDocument {
  return {
    v: 1,
    type: template.id,
    title,
    sections: template.sections.map((section) => ({
      name: section.name,
      fields: section.fields.map((field): Field => emptyField(field)),
    })),
  };
}

function emptyField({ label, type }: TemplateField): Field {
  return { label, type, value: null };
}
