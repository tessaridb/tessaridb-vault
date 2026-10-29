// Fetching what a revision of a shared record wraps its new key to, right before it is sealed: the record's shares,
// the key directory, and each group's public key.

import { call } from "../api.ts";
import { hex, type Bytes } from "../crypto/bytes.ts";
import { readGroupKey } from "../records-model.ts";
import type { Unlocked } from "../vault.ts";
import { loadFailure } from "./list-view.ts";
import { companyPath } from "./load.ts";
import type { ShareRecipient } from "./save-record.ts";
import { readDirectory, readShares, revisionRecipients } from "./shares-model.ts";

const MOVED = "This record's sharing changed while it was open. Open it again, then make your change.";

/** The recipients record `id`'s next revision wraps to, or the words for why they could not be had. */
export async function sharedWith(
  held: Unlocked,
  id: string,
): Promise<{ readonly ok: true; readonly recipients: readonly ShareRecipient[] } | { readonly ok: false; readonly words: string }> {
  const path = companyPath(held);
  const listed = await call("GET", `${path}/records/${encodeURIComponent(id)}/shares`, readShares);
  if (!listed.ok) {
    return { ok: false, words: listed.status === 403 ? "You can read this record but not change it." : loadFailure(listed) };
  }
  const directory = await call("GET", `${path}/keys`, readDirectory);
  if (!directory.ok) {
    return { ok: false, words: loadFailure(directory) };
  }
  const groups = new Map<string, Bytes>();
  for (const share of listed.value.shares.filter((each) => each.kind === "group")) {
    const key = await call("GET", `${path}/groups/${encodeURIComponent(share.target)}/key`, readGroupKey);
    if (!key.ok) {
      return { ok: false, words: key.status === 404 ? MOVED : loadFailure(key) };
    }
    groups.set(share.target, key.value.public);
  }
  const recipients = revisionRecipients(listed.value, directory.value, groups, hex(held.keys.keyId));
  return recipients === null ? { ok: false, words: MOVED } : { ok: true, recipients };
}
