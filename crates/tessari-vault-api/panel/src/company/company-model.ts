// Deleting a company, without a DOM: the statement an owner's browser signs with their own signing key, so the server
// can never ask on an owner's behalf; what the server says about a deletion already asked for; and when the page lets
// the delete button be pressed.
//
// Statement bytes: "tessaridb-vault/v1/company-drop" ‖ [version] ‖ [act] ‖ lp(company) ‖ u64 at, `at` in seconds.

import type { Failure } from "../api.ts";
import { encode } from "../base64url.ts";
import { concat, field, integer, utf8, type Bytes } from "../crypto/bytes.ts";
import { FORMAT_VERSION } from "../crypto/labels.ts";
import { sign } from "../crypto/signatures.ts";

const LABEL = "tessaridb-vault/v1/company-drop";

export type DropAct = "request" | "cancel";

const ACT_BYTE: Readonly<Record<DropAct, number>> = { request: 1, cancel: 2 };

export function dropStatement(act: DropAct, company: string, at: number): Bytes {
  return concat(utf8(LABEL), Uint8Array.of(FORMAT_VERSION, ACT_BYTE[act]), field(utf8(company)), integer(at, 8));
}

/** The body a request or a cancel is sent with: the time it names and the owner's signature over it. */
export async function signedDrop(signingPrivate: CryptoKey, act: DropAct, company: string, now: () => number = Date.now): Promise<{ readonly at: number; readonly signature: string }> {
  const at = Math.floor(now() / 1000);
  return { at, signature: encode(await sign(signingPrivate, dropStatement(act, company, at))) };
}

/** A deletion an owner asked for: waiting for the platform admin, or confirmed and carried out after `dropAfter`. */
export type Standing = { readonly state: "requested" } | { readonly state: "scheduled"; readonly dropAfter: number };

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

export function readStanding(v: unknown): Standing | null {
  if (!isRecord(v)) {
    return null;
  }
  if (v["state"] === "requested") {
    return { state: "requested" };
  }
  const after = v["drop_after"];
  return v["state"] === "scheduled" && typeof after === "number" && Number.isSafeInteger(after) ? { state: "scheduled", dropAfter: after } : null;
}

/** Whether the person typed the company's exact name and gave a reason, which is when a deletion may be asked for. */
export const deletionReady = (typed: string, reason: string, companyName: string): boolean => typed.trim() === companyName && reason.trim() !== "";

/** A refused deletion step in words: what happened and what to do next. */
export function dropFailure(failure: Failure): string {
  switch (failure.code) {
    case "drop_unsigned":
      return "The server didn't take your signature: your key for this company isn't the one it holds, or this device's clock is more than ten minutes off. Check the clock, unlock again, then try again.";
    case "drop_scheduled":
      return "The deletion is already confirmed and waiting. Cancel it first if you want to ask again.";
    case "forbidden":
      return "Only an owner can delete the company or cancel its deletion.";
    case "not_found":
      return "No deletion is waiting any more. The page is shown again.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    default:
      return "The vault couldn't make this change. Try again; if it keeps failing, contact the platform admin.";
  }
}
