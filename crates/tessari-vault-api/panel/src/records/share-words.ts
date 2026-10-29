// The words for a refused share or share removal, kept apart from the dialogs so they can be checked without a page.

import type { Refusal } from "../crypto/errors.ts";

const REOPEN = "This record or its sharing changed while you were deciding. Open it again, then share it.";

/** What to say when sharing a record answered `status` and `code`; `fallback` covers everything else. */
export function shareFailure(status: number, code: string, fallback: string): string {
  if (status === 403) {
    return "You can read this record but not share it.";
  }
  if (status !== 409) {
    return fallback;
  }
  switch (code) {
    case "already_shared":
      return "This record is shared with them already.";
    case "no_key":
      return "They have no key yet, so nothing can be shared with them. Ask them to set their unlock password first.";
    case "unknown_key":
      return "Their key changed while you were deciding. Open the record again, then share it.";
    case "stale_revision":
    case "stale_shares":
      return REOPEN;
    default:
      return fallback;
  }
}

/** What to say when this page would not seal a share: a key the company's chain does not vouch for, or anything else. */
export function sealFailure(reason: Refusal): string {
  return reason === "unvouched-recipient"
    ? "Their key is not vouched for in the company yet, so nothing can be shared with them. An admin vouches for new keys."
    : "This record could not be opened to share it. Open it again, then share it.";
}

/** What to say when removing a share answered `status` and `code`; `fallback` covers everything else. */
export function removeShareFailure(status: number, _code: string, fallback: string): string {
  if (status === 403) {
    return "You can read this record but not change who it is shared with.";
  }
  if (status === 404) {
    return "The record is no longer shared with them, or it is gone.";
  }
  return fallback;
}
