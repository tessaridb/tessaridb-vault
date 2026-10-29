// Sharing a record from the browser: the latest revision's record key comes out of the reader's own wrap, is wrapped
// to the target's key only when the owner's chain vouches for it, and is wiped. Only the wrap leaves the page.

import { encode } from "../base64url.ts";
import { hex, wipe } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { wrapToVouched } from "../crypto/recipients.ts";
import type { Vouched } from "../crypto/statements.ts";
import type { OpenedRecord } from "../records-model.ts";
import { keyIdBytes } from "../unlock-model.ts";
import { recordKeyOf, type RecipientKey } from "./open-record.ts";
import type { ShareRecipient } from "./save-record.ts";

/** The body `POST /api/companies/{company}/records/{record}/shares` takes to share `opened` with `to` for `right`. */
export async function shareBody(
  company: string,
  vouched: Vouched,
  reader: RecipientKey,
  opened: OpenedRecord,
  to: ShareRecipient,
  right: "read" | "edit",
): Promise<Readonly<Record<string, unknown>>> {
  const raw = await recordKeyOf(company, vouched, reader, opened);
  try {
    const wrappedKeyId = keyIdBytes(opened.row.keyId);
    if (wrappedKeyId === null) {
      throw new VaultCryptoError("key-mismatch");
    }
    const context = { company, wrappedKind: "record", wrappedKeyId, recipientKind: to.recipient.kind, recipientKeyId: to.keyId } as const;
    const wrap = await wrapToVouched(vouched, to.recipient, to.listed, context, raw);
    const target = to.recipient.kind === "member" ? { member: to.recipient.account } : { group: to.recipient.group };
    return { ...target, right, key_id: hex(to.keyId), wrap: encode(wrap) };
  } finally {
    wipe(raw);
  }
}
