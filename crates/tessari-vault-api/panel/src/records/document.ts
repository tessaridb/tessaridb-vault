// A record's content: `{v: 1, type, title, sections: [{name, fields}]}` as UTF-8 JSON, which is the plaintext the
// record cipher seals. The title, section names and labels are inside it, so the server never sees them. Decoding is
// strict and bounded, because the bytes come out of a decryption a hostile writer may have produced.

import { utf8, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { readField, readText, type Field } from "./fields.ts";

export interface RecordSection {
  /** Empty for the record's main section. */
  name: string;
  fields: Field[];
}

export interface RecordDocument {
  v: 1;
  /** A built-in type id or a company-defined type's id. */
  type: string;
  title: string;
  sections: RecordSection[];
}

const MAX_SECTIONS = 64;
const MAX_FIELDS = 512;
const TYPE_ID = /^[a-z0-9][a-z0-9:-]{0,63}$/;

function fail(): never {
  throw new VaultCryptoError("malformed");
}

function object(raw: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    return fail();
  }
  const present = Object.keys(raw);
  if (present.length !== keys.length || !keys.every((key) => present.includes(key))) {
    return fail();
  }
  return Object.fromEntries(Object.entries(raw));
}

export function encodeRecord(document: RecordDocument): Bytes {
  return utf8(JSON.stringify(document));
}

export function decodeRecord(bytes: Bytes): RecordDocument {
  let raw: unknown;
  try {
    raw = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    return fail();
  }
  if (typeof raw === "object" && raw !== null && "v" in raw && raw.v !== 1) {
    throw new VaultCryptoError("unknown-version");
  }
  const document = object(raw, ["v", "type", "title", "sections"]);
  const type = readText(document["type"]);
  const sections = document["sections"];
  if (!TYPE_ID.test(type) || !Array.isArray(sections)) {
    return fail();
  }
  if (sections.length > MAX_SECTIONS) {
    throw new VaultCryptoError("too-large");
  }
  let fieldCount = 0;
  const read = sections.map((rawSection: unknown): RecordSection => {
    const section = object(rawSection, ["name", "fields"]);
    const fields = section["fields"];
    if (!Array.isArray(fields)) {
      return fail();
    }
    fieldCount += fields.length;
    if (fieldCount > MAX_FIELDS) {
      throw new VaultCryptoError("too-large");
    }
    return { name: readText(section["name"]), fields: fields.map(readField) };
  });
  return { v: 1, type, title: readText(document["title"]), sections: read };
}
