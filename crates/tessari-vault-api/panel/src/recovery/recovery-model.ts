// Recovery (C7) besides the pages: what the recovery routes answer, the bodies the browser sends, when a member's keys
// are escrowed again, which notices this browser has not shown yet, and the words for each refusal. No DOM here, so all
// of it is tested.

import { decode, encode } from "../base64url.ts";
import type { Failure } from "../api.ts";
import type { Bytes } from "../crypto/bytes.ts";
import type { StoredKeySet } from "../crypto/key-set.ts";
import type { RecoveryKey } from "../crypto/statements.ts";

const KEY_BYTES = 32;

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isKeyId = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";

/** Non-empty base64url bytes, of `length` when given. */
function bytesOf(v: unknown, length?: number): Bytes | null {
  const bytes = isText(v) ? decode(v) : null;
  return bytes === null || bytes.length === 0 || (length !== undefined && bytes.length !== length) ? null : bytes;
}

/** `GET /api/companies/{company}/keys/me/escrow`: the recovery key the caller's escrow is wrapped to. */
export const readOwnEscrow = (v: unknown): string | null => (isRecord(v) && isKeyId(v["recovery_key_id"]) ? v["recovery_key_id"] : null);

/** `GET /api/companies/{company}/recovery-key`: the company recovery key as the server lists it. */
export function readRecoveryKey(v: unknown): { readonly keyId: string; readonly public: Bytes } | null {
  const key = isRecord(v) && isKeyId(v["key_id"]) ? v["key_id"] : null;
  const listed = isRecord(v) ? bytesOf(v["public"], KEY_BYTES) : null;
  return key === null || listed === null ? null : { keyId: key, public: listed };
}

/** `GET /api/companies/{company}/recovery-key/mine` and `GET /api/companies/{company}/keys/{account}/escrow`: a wrap and the key id it is named by. */
export function readWrap(idField: "key_id" | "recovery_key_id"): (v: unknown) => { readonly keyId: string; readonly wrap: Bytes } | null {
  return (v) => {
    const key = isRecord(v) && isKeyId(v[idField]) ? v[idField] : null;
    const wrap = isRecord(v) ? bytesOf(v["wrap"]) : null;
    return key === null || wrap === null ? null : { keyId: key, wrap };
  };
}

/** A recovery request no owner has handed over yet. */
export interface OpenRequest {
  readonly id: string;
  readonly account: string;
  readonly handoffPublic: Bytes;
  /** Seconds since the epoch. */
  readonly created: number;
}

/** `GET /api/companies/{company}/recovery-requests`. */
export function readRequests(v: unknown): readonly OpenRequest[] | null {
  if (!isRecord(v) || !Array.isArray(v["requests"])) {
    return null;
  }
  const requests: OpenRequest[] = [];
  for (const item of v["requests"]) {
    const handoffPublic = isRecord(item) ? bytesOf(item["handoff_public"], KEY_BYTES) : null;
    if (!isRecord(item) || !isKeyId(item["id"]) || !isText(item["account"]) || handoffPublic === null || !Number.isSafeInteger(item["created"])) {
      return null;
    }
    requests.push({ id: item["id"], account: item["account"], handoffPublic, created: Number(item["created"]) });
  }
  return requests;
}

/** `POST /api/companies/{company}/recovery-requests`: the request's id. */
export const readAsked = (v: unknown): string | null => (isRecord(v) && isKeyId(v["id"]) ? v["id"] : null);

/** `POST /api/companies/{company}/recovery-requests/{id}/redeem`: the owner's handoff wrap. */
export const readHandoff = (v: unknown): Bytes | null => (isRecord(v) ? bytesOf(v["wrap"]) : null);

/** A notice about the caller: an owner read their escrow (`escrow.read`) or handed their keys over (`recovery`). */
export interface Notice {
  readonly by: string;
  readonly kind: string;
  /** Seconds since the epoch. */
  readonly at: number;
}

/** `GET /api/companies/{company}/notices/me`, oldest first. */
export function readNotices(v: unknown): readonly Notice[] | null {
  if (!isRecord(v) || !Array.isArray(v["notices"])) {
    return null;
  }
  const notices: Notice[] = [];
  for (const item of v["notices"]) {
    if (!isRecord(item) || !isText(item["by"]) || !isText(item["kind"]) || !Number.isSafeInteger(item["at"])) {
      return null;
    }
    notices.push({ by: item["by"], kind: item["kind"], at: Number(item["at"]) });
  }
  return notices;
}

/** Whether to escrow again: the chain vouches a recovery key and the escrow is not wrapped to it. */
export const escrowWanted = (escrowedTo: string | null, vouched: RecoveryKey | undefined): boolean =>
  vouched !== undefined && vouched.keyId !== escrowedTo;

/** The notices after `seenAt`, the newest instant this browser already showed; all of them when it showed none. */
export const newNotices = (notices: readonly Notice[], seenAt: number | null): readonly Notice[] =>
  seenAt === null ? notices : notices.filter((notice) => notice.at > seenAt);

/** What a notice says happened, by whom. */
export function noticeWords(kind: string, by: string): string {
  switch (kind) {
    case "escrow.read":
      return `${by} read your keys kept for recovery`;
    case "recovery":
      return `${by} handed your keys back to you`;
    default:
      return `${by}: ${kind}`;
  }
}

export const escrowBody = (recoveryKeyId: string, wrap: Bytes): Readonly<Record<string, unknown>> => ({ recovery_key_id: recoveryKeyId, wrap: encode(wrap) });

export const askBody = (handoffPublic: Bytes): Readonly<Record<string, unknown>> => ({ handoff_public: encode(handoffPublic) });

export const handoffBody = (wrap: Bytes): Readonly<Record<string, unknown>> => ({ wrap: encode(wrap) });

/** `POST /api/companies/{company}/keys/me/rewrap`: the restored key set's wrapped keys and parameters, using up request `request`. */
export const rewrapBody = (request: string, stored: StoredKeySet): Readonly<Record<string, unknown>> => ({
  request,
  kdf: { salt: encode(stored.salt), memory_kib: stored.kdf.memoryKib, passes: stored.kdf.passes, lanes: stored.kdf.lanes },
  wrapped: encode(stored.wrapped),
});

/** What went wrong in a recovery step and what to do. */
export function recoveryFailure(failure: Failure): string {
  switch (failure.code) {
    case "no_escrow":
      return "Your keys aren't kept for recovery in this company yet, so an owner can't restore them. Ask your admin.";
    case "not_handed_over":
      return "An owner hasn't handed your keys back yet. Check again later; the request stays open for 3 days.";
    case "recovery_closed":
      return "This recovery was used already or has expired. Ask again.";
    case "recovery_keyed":
      return "The company already has a recovery key. Reload the page to see it.";
    case "forbidden":
      return "Only the company's owners can do this.";
    case "no_owner_wrap":
      return "You have no copy of the recovery key yet: it was made before you were an owner. An owner who has one can give you yours on this page.";
    case "outside_reach":
      return "The owners waiting for the recovery key changed while this page was open. Reload the page and try again.";
    case "recovery_moved":
      return "The recovery key changed while this page was open. Reload the page and try again.";
    case "unopenable":
      return "These keys didn't open with the recovery key. Nothing was handed over; tell the member to ask again.";
    case "unvouched-signer":
      return "Only the company owner key can vouch for the recovery key, and this browser doesn't hold it.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    default:
      return "The vault couldn't do that. Try again; if it keeps failing, tell your admin.";
  }
}
