// What the records views read: a record's row, an opened record (its row and its latest revision's sealed parts with
// the record key wrapped to a key this person holds), a page of opened records, and the group keys a wrap may be kept
// under. No DOM here, so all of it is tested.

import { decode } from "./base64url.ts";
import type { Bytes } from "./crypto/bytes.ts";
import type { ListedGroupKey } from "./records/recipient-keys.ts";
import { keyIdBytes } from "./unlock-model.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";
const isId = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
const isWhole = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;

/** A record's row: who wrote it, which revision, under which key; nothing of what it holds. */
export interface RecordRow {
  readonly id: string;
  readonly recordType: string;
  readonly revision: number;
  readonly keyId: string;
  readonly author: string;
  readonly created: number;
  readonly updated: number;
  readonly rotationDue: boolean;
}

export function readRow(v: unknown): RecordRow | null {
  if (!isRecord(v)) {
    return null;
  }
  const { id, record_type: recordType, revision, key_id: keyId, author, created, updated, rotation_due: rotationDue } = v;
  return isId(id) && isText(recordType) && isWhole(revision) && revision > 0 && isId(keyId) && isId(author) && isWhole(created) && isWhole(updated) && typeof rotationDue === "boolean"
    ? { id, recordType, revision, keyId, author, created, updated, rotationDue }
    : null;
}

/** Whom the wrap in an opened record is kept under. */
export type Recipient = { readonly kind: "member" | "group"; readonly keyId: string };

export function readRecipient(v: unknown): Recipient | null {
  if (typeof v !== "string") {
    return null;
  }
  const [kind, keyId, extra] = v.split(":");
  return (kind === "member" || kind === "group") && isId(keyId) && extra === undefined ? { kind, keyId } : null;
}

/** A record as it is opened: its row and its latest revision's sealed parts. */
export interface OpenedRecord {
  readonly row: RecordRow;
  readonly body: Bytes;
  readonly signature: Bytes;
  readonly authorKey: string;
  /** The account that wrote the latest revision, whose key signed it. */
  readonly revisedBy: string;
  readonly recipient: Recipient;
  readonly wrap: Bytes;
  /** The ids of the person's own groups the record is shared with. */
  readonly through: readonly string[];
}

export function readOpened(v: unknown): OpenedRecord | null {
  if (!isRecord(v)) {
    return null;
  }
  const row = readRow(v["record"]);
  const recipient = readRecipient(v["recipient"]);
  const [body, signature, wrap] = [v["body"], v["signature"], v["wrap"]].map((part) => (isText(part) ? decode(part) : null));
  const authorKey = v["author_key"];
  const through = v["through"];
  const revisedBy = v["revised_by"];
  return row !== null && recipient !== null && body != null && signature != null && wrap != null && isId(authorKey) && isId(revisedBy) && Array.isArray(through) && through.every(isId)
    ? { row, body, signature, authorKey, revisedBy, recipient, wrap, through }
    : null;
}

/** A page of opened records and the cursor to the next, null on the last page. */
export interface OpenedPage {
  readonly records: readonly OpenedRecord[];
  readonly next: string | null;
}

export function readOpenedPage(v: unknown): OpenedPage | null {
  if (!isRecord(v) || !Array.isArray(v["records"])) {
    return null;
  }
  const next = v["next"];
  if (next !== null && !isId(next)) {
    return null;
  }
  const records: OpenedRecord[] = [];
  for (const item of v["records"]) {
    const opened = readOpened(item);
    if (opened === null) {
      return null;
    }
    records.push(opened);
  }
  return { records, next };
}

/** The route of a page of opened records in `company`, after `cursor` when given. */
export const openedPagePath = (company: string, cursor: string | null): string =>
  `/api/companies/${encodeURIComponent(company)}/records?opened=true${cursor === null ? "" : `&after=${encodeURIComponent(cursor)}`}`;

/** The ids of the company's groups, as `GET /api/companies/{company}/groups` answers them. */
export function readGroupIds(v: unknown): readonly string[] | null {
  if (!isRecord(v) || !Array.isArray(v["groups"])) {
    return null;
  }
  const ids = v["groups"].map((group: unknown) => (isRecord(group) && isId(group["id"]) ? group["id"] : null));
  return ids.every((id) => id !== null) ? ids : null;
}

/**
 * A group's key as `GET /api/companies/{company}/groups/{group}/key` answers it; `wrap` is null when none is kept for the
 * caller.
 */
export function readGroupKey(v: unknown): ListedGroupKey | null {
  if (!isRecord(v) || typeof v["key_id"] !== "string" || !isText(v["public"])) {
    return null;
  }
  const keyId = keyIdBytes(v["key_id"]);
  const listed = decode(v["public"]);
  const wrapped = v["wrap"];
  const wrap = wrapped === null ? null : isText(wrapped) ? (decode(wrapped) ?? undefined) : undefined;
  return keyId !== null && listed !== null && listed.length === 32 && wrap !== undefined ? { keyId, public: listed, wrap } : null;
}
