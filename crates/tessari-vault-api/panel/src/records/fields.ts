// The fourteen field types a record holds and the exact shape of each value. A decrypted record is input like any
// other, so `readField` accepts only these shapes: an extra key, a wrong type or an out-of-range value is `malformed`.
// Every value may be null, which is a field created from a template and not filled in yet.

import { VaultCryptoError } from "../crypto/errors.ts";
import { FILE_MAX_BYTES } from "../crypto/files.ts";
import { base32Decode, type OtpAlgorithm } from "./totp.ts";

export const FIELD_TYPES = [
  "text",
  "multiline",
  "concealed",
  "url",
  "email",
  "phone",
  "number",
  "date",
  "month-year",
  "boolean",
  "otp",
  "file",
  "security-question",
  "ssh-key",
  "concealed-multiline",
] as const;

export type FieldType = (typeof FIELD_TYPES)[number];

export interface OtpValue {
  /** The seed in base32, as the issuer printed it. */
  secret: string;
  algorithm: OtpAlgorithm;
  digits: 6 | 8;
  period: number;
}

/**
 * A file on the record: the server's id for its sealed bytes, what it is, and its own key. The key travels inside the
 * record's sealed body, so every revision carries it to every reader and the file is sealed once, whatever the
 * record's revision key; `key` is 32 bytes and `keyId`, which the file's header names, 16, both in lower-case hex.
 */
export interface FileValue {
  id: string;
  name: string;
  size: number;
  mediaType: string;
  key: string;
  keyId: string;
}

export interface SecurityQuestionValue {
  question: string;
  answer: string;
}

export interface SshKeyValue {
  privateKey: string;
  publicKey: string;
  fingerprint: string;
}

type TextType = "text" | "multiline" | "concealed" | "concealed-multiline" | "url" | "email" | "phone" | "number" | "date" | "month-year";

export type Field =
  | { label: string; type: TextType; value: string | null }
  | { label: string; type: "boolean"; value: boolean | null }
  | { label: string; type: "otp"; value: OtpValue | null }
  | { label: string; type: "file"; value: FileValue | null }
  | { label: string; type: "security-question"; value: SecurityQuestionValue | null }
  | { label: string; type: "ssh-key"; value: SshKeyValue | null };

/** The longest string any one value or label may hold, in UTF-16 code units. */
export const MAX_TEXT = 65_536;

const SHAPES: Record<Exclude<TextType, "text" | "multiline" | "concealed" | "concealed-multiline">, (text: string) => boolean> = {
  url: (text) => URL.canParse(text),
  email: (text) => /^[^\s@]+@[^\s@]+$/.test(text),
  phone: (text) => /^[+0-9 ().-]{1,32}$/.test(text),
  number: (text) => /^-?\d+(\.\d+)?$/.test(text),
  date: (text) => {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
    if (match === null) {
      return false;
    }
    const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
    const date = new Date(Date.UTC(year, month - 1, day));
    return date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
  },
  "month-year": (text) => /^\d{4}-(0[1-9]|1[0-2])$/.test(text),
};

function fail(): never {
  throw new VaultCryptoError("malformed");
}

/** An object with exactly `keys`, nothing more and nothing less. */
function exactly(raw: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    return fail();
  }
  const present = Object.keys(raw);
  if (present.length !== keys.length || !keys.every((key) => present.includes(key))) {
    return fail();
  }
  return Object.fromEntries(Object.entries(raw));
}

export function readText(raw: unknown): string {
  if (typeof raw !== "string") {
    return fail();
  }
  if (raw.length > MAX_TEXT) {
    throw new VaultCryptoError("too-large");
  }
  return raw;
}

function readOtp(raw: unknown): OtpValue {
  const value = exactly(raw, ["secret", "algorithm", "digits", "period"]);
  const secret = readText(value["secret"]);
  const { algorithm, digits, period } = value;
  if (
    base32Decode(secret).length === 0 ||
    (algorithm !== "SHA1" && algorithm !== "SHA256" && algorithm !== "SHA512") ||
    (digits !== 6 && digits !== 8) ||
    typeof period !== "number" ||
    !Number.isInteger(period) ||
    period < 1 ||
    period > 3600
  ) {
    return fail();
  }
  return { secret, algorithm, digits, period };
}

const hexOf = (raw: unknown, bytes: number): string => (typeof raw === "string" && new RegExp(`^[0-9a-f]{${2 * bytes}}$`).test(raw) ? raw : fail());

function readFile(raw: unknown): FileValue {
  const value = exactly(raw, ["id", "name", "size", "mediaType", "key", "keyId"]);
  const size = value["size"];
  if (typeof size !== "number" || !Number.isInteger(size) || size < 0 || size > FILE_MAX_BYTES) {
    return fail();
  }
  return {
    id: readText(value["id"]),
    name: readText(value["name"]),
    size,
    mediaType: readText(value["mediaType"]),
    key: hexOf(value["key"], 32),
    keyId: hexOf(value["keyId"], 16),
  };
}

function orNull<T>(raw: unknown, read: (raw: unknown) => T): T | null {
  return raw === null ? null : read(raw);
}

function readShaped(type: Exclude<TextType, "text" | "multiline" | "concealed" | "concealed-multiline">, raw: unknown): string {
  const text = readText(raw);
  return text === "" || SHAPES[type](text) ? text : fail();
}

function readSecurityQuestion(raw: unknown): SecurityQuestionValue {
  const value = exactly(raw, ["question", "answer"]);
  return { question: readText(value["question"]), answer: readText(value["answer"]) };
}

function readSshKey(raw: unknown): SshKeyValue {
  const value = exactly(raw, ["privateKey", "publicKey", "fingerprint"]);
  return {
    privateKey: readText(value["privateKey"]),
    publicKey: readText(value["publicKey"]),
    fingerprint: readText(value["fingerprint"]),
  };
}

function isFieldType(raw: unknown): raw is FieldType {
  return FIELD_TYPES.some((type) => type === raw);
}

/** One field from decrypted JSON: `{label, type, value}` exactly, the value of its type's shape or null. */
export function readField(raw: unknown): Field {
  const field = exactly(raw, ["label", "type", "value"]);
  const { type, value } = field;
  if (!isFieldType(type)) {
    return fail();
  }
  const label = readText(field["label"]);
  switch (type) {
    case "text":
    case "multiline":
    case "concealed":
    case "concealed-multiline":
      return { label, type, value: orNull(value, readText) };
    case "url":
    case "email":
    case "phone":
    case "number":
    case "date":
    case "month-year":
      return { label, type, value: orNull(value, (text) => readShaped(type, text)) };
    case "boolean":
      return { label, type, value: orNull(value, (flag) => (typeof flag === "boolean" ? flag : fail())) };
    case "otp":
      return { label, type, value: orNull(value, readOtp) };
    case "file":
      return { label, type, value: orNull(value, readFile) };
    case "security-question":
      return { label, type, value: orNull(value, readSecurityQuestion) };
    case "ssh-key":
      return { label, type, value: orNull(value, readSshKey) };
  }
}
