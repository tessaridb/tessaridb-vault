// Record content: the title, sections, labels and values a record holds, encoded as the plaintext the record cipher
// seals. Every built-in type round-trips through the cipher; every field type accepts its shape and refuses others.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { hex, random, utf8 } from "../../src/crypto/bytes.ts";
import { importRecordKey, openRevision, sealRevision } from "../../src/crypto/record.ts";
import { decodeRecord, encodeRecord, type RecordDocument } from "../../src/records/document.ts";
import { FIELD_TYPES, type Field, type FieldType } from "../../src/records/fields.ts";
import { BUILT_IN_TYPES, recordFromTemplate } from "../../src/records/types.ts";

/** A valid value for every field type. */
const SAMPLE: Record<FieldType, Field["value"]> = {
  text: "admin",
  multiline: "line one\nline two",
  concealed: "correct horse battery staple",
  url: "https://mail.example.com/login",
  email: "ops@example.com",
  phone: "+1 (555) 010-0199",
  number: "5432",
  date: "2027-02-28",
  "month-year": "2028-11",
  boolean: true,
  otp: { secret: "JBSWY3DPEHPK3PXP", algorithm: "SHA1", digits: 6, period: 30 },
  file: { id: "file-1", name: "contract.pdf", size: 1024, mediaType: "application/pdf", key: "ab".repeat(32), keyId: "cd".repeat(16) },
  "security-question": { question: "First school?", answer: "Hillside" },
  "ssh-key": { privateKey: "-----BEGIN OPENSSH PRIVATE KEY-----\nbody", publicKey: "ssh-ed25519 AAAA test", fingerprint: "SHA256:abc" },
};

/** An invalid value for every field type. */
const WRONG: Record<FieldType, unknown> = {
  text: 7,
  multiline: false,
  concealed: ["x"],
  url: "not a url",
  email: "no-at-sign",
  phone: "call me",
  number: "1e5",
  date: "2027-02-30",
  "month-year": "2028-13",
  boolean: "true",
  otp: { secret: "JBSWY3DPEHPK3PXP", algorithm: "MD5", digits: 6, period: 30 },
  file: { id: "file-1", name: "a.pdf", size: -1, mediaType: "application/pdf" },
  "security-question": { question: "First school?" },
  "ssh-key": { privateKey: "x", publicKey: "y", fingerprint: "z", extra: 1 },
};

function filled(document: RecordDocument): RecordDocument {
  return {
    ...document,
    sections: document.sections.map((section) => ({
      ...section,
      fields: section.fields.map((field) => ({ ...field, value: SAMPLE[field.type] }) as Field),
    })),
  };
}

function withField(type: string, value: unknown): unknown {
  return { v: 1, type: "secure-note", title: "t", sections: [{ name: "", fields: [{ label: "l", type, value }] }] };
}

describe("record content", () => {
  it("names the eleven built-in types", () => {
    assert.deepEqual(BUILT_IN_TYPES.map((type) => type.id), [
      "login",
      "secure-note",
      "ssh-key",
      "file",
      "payment-card",
      "identity",
      "server",
      "database",
      "api-credential",
      "wireless-network",
      "software-license",
    ]);
  });

  it("round-trips every built-in type, filled, through the record cipher", async () => {
    const keyId = random(16);
    const key = await importRecordKey(random(32));
    const keys = new Map([[hex(keyId), key]]);
    assert.equal(BUILT_IN_TYPES.length, 11, "the loop below covers every built-in type");
    for (const [index, template] of BUILT_IN_TYPES.entries()) {
      const document = filled(recordFromTemplate(template, `A ${template.id}`));
      const context = { company: "c", record: `r-${index}`, revision: 1, keyId };
      const body = await sealRevision(key, context, encodeRecord(document));
      assert.deepEqual(decodeRecord(await openRevision(keys, context, body)), document, template.id);
    }
  });

  it("starts a record from a template with every field empty", () => {
    const login = BUILT_IN_TYPES[0];
    assert.ok(login);
    const document = recordFromTemplate(login, "Mail");
    assert.equal(document.type, "login");
    assert.ok(document.sections.flatMap((section) => section.fields).every((field) => field.value === null));
  });

  it("keeps added fields of every type in named sections", () => {
    const document: RecordDocument = {
      v: 1,
      type: "server",
      title: "db-1",
      sections: [{ name: "Extra", fields: FIELD_TYPES.map((type) => ({ label: type, type, value: SAMPLE[type] }) as Field) }],
    };
    assert.deepEqual(decodeRecord(encodeRecord(document)), document);
  });

  for (const type of FIELD_TYPES) {
    it(`refuses a ${type} field holding the wrong shape`, () => {
      assert.throws(() => decodeRecord(utf8(JSON.stringify(withField(type, WRONG[type])))), /malformed/);
    });
  }

  it("keeps a file's own key in its field, and refuses a file without a whole one", () => {
    const file = { id: "file-1", name: "a.pdf", size: 10, mediaType: "application/pdf" };
    const kept = decodeRecord(utf8(JSON.stringify(withField("file", { ...file, key: "ab".repeat(32), keyId: "cd".repeat(16) }))));
    assert.deepEqual(kept.sections[0]?.fields[0]?.value, { ...file, key: "ab".repeat(32), keyId: "cd".repeat(16) });
    assert.throws(() => decodeRecord(utf8(JSON.stringify(withField("file", file)))), /malformed/);
    assert.throws(() => decodeRecord(utf8(JSON.stringify(withField("file", { ...file, key: "ab".repeat(31), keyId: "cd".repeat(16) })))), /malformed/);
    assert.throws(() => decodeRecord(utf8(JSON.stringify(withField("file", { ...file, key: "AB".repeat(32), keyId: "cd".repeat(16) })))), /malformed/);
    assert.throws(() => decodeRecord(utf8(JSON.stringify(withField("file", { ...file, key: "ab".repeat(32), keyId: "cd".repeat(15) })))), /malformed/);
  });

  it("refuses an unknown field type, an unknown version and a record that is not JSON", () => {
    assert.throws(() => decodeRecord(utf8(JSON.stringify(withField("password", "x")))), /malformed/);
    assert.throws(() => decodeRecord(utf8(JSON.stringify({ v: 2, type: "login", title: "t", sections: [] }))), /unknown-version/);
    assert.throws(() => decodeRecord(utf8("{not json")), /malformed/);
    assert.throws(() => decodeRecord(utf8('{"v":1,"type":"login","title":"t","sections":[],"extra":1}')), /malformed/);
  });

  it("refuses a record above its ceilings", () => {
    const many = { v: 1, type: "login", title: "t", sections: [{ name: "", fields: Array.from({ length: 513 }, () => ({ label: "l", type: "text", value: "" })) }] };
    assert.throws(() => decodeRecord(utf8(JSON.stringify(many))), /too-large/);
    const long = withField("multiline", "x".repeat(65_537));
    assert.throws(() => decodeRecord(utf8(JSON.stringify(long))), /too-large/);
  });
});
