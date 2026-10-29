// The members page's words, kept apart from the page so they can be checked without one.

import type { Refusal } from "../crypto/errors.ts";
import type { KeyState } from "./members-model.ts";

/** A key state in words. */
export function keyStateWords(state: KeyState): string {
  switch (state) {
    case "vouched":
      return "Key vouched for";
    case "waiting":
      return "Key waiting to be vouched for";
    case "no-key":
      return "Hasn't set an unlock password yet";
  }
}

/** What to say when vouching answered `status` and `code`; `fallback` covers everything else. */
export function vouchFailure(status: number, code: string, fallback: string): string {
  if (status === 403) {
    return "Only the company's owner or an admin can vouch for keys.";
  }
  if (status === 409 && code === "stale_chain") {
    return "Someone vouched for a key meanwhile. The list is shown again; vouch once more.";
  }
  return fallback;
}

/** What to say when this page would not vouch, because the chain with the new statement would not verify. */
export function vouchRefused(reason: Refusal): string {
  return reason === "unvouched-signer"
    ? "Only the company's owner or an admin the owner vouched for can vouch for keys, and your key isn't one of them yet."
    : "Your company's list of who may read what doesn't check out, so nothing was vouched for. Tell your company's owner.";
}
