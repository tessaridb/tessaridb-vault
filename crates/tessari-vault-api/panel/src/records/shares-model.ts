// What a revision of a shared record needs before it is sealed: the record's shares as the server lists them to an
// editor, the members' public keys from the company's key directory and each group's public key, put together into
// the recipients its new record key is wrapped to. No DOM here, so all of it is tested. Every key still passes the
// chain check in `wrapToVouched` when the revision is sealed.

import { decode } from "../base64url.ts";
import type { Bytes } from "../crypto/bytes.ts";
import { keyIdBytes } from "../unlock-model.ts";
import type { ShareRecipient } from "./save-record.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isId = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);

/** One share: the member or group it reaches, what it lets them do, and the id of the key it wraps to. */
export interface ListedShare {
  readonly kind: "member" | "group";
  readonly target: string;
  readonly right: "read" | "edit";
  readonly keyId: string;
}

/** A record's author, with the key set they keep (null when none), and its shares. */
export interface ListedShares {
  readonly author: { readonly account: string; readonly keyId: string | null };
  readonly shares: readonly ListedShare[];
}

/** The answer of `GET /api/companies/{company}/records/{record}/shares`. */
export function readShares(v: unknown): ListedShares | null {
  if (!isRecord(v) || !isRecord(v["author"]) || !Array.isArray(v["shares"])) {
    return null;
  }
  const { account, key_id: authorKey } = v["author"];
  if (!isId(account) || (authorKey !== null && !isId(authorKey))) {
    return null;
  }
  const shares = v["shares"].map((share: unknown): ListedShare | null => {
    if (!isRecord(share) || !isId(share["target"]) || !isId(share["key_id"])) {
      return null;
    }
    const { kind, right } = share;
    return (kind === "member" || kind === "group") && (right === "read" || right === "edit")
      ? { kind, target: share["target"], right, keyId: share["key_id"] }
      : null;
  });
  return shares.every((share) => share !== null) ? { author: { account, keyId: authorKey }, shares } : null;
}

/** A member's listed key set: its id and its public agreement key. */
export interface DirectoryKey {
  readonly keyId: string;
  readonly agreementPublic: Bytes;
}

/** The answer of `GET /api/companies/{company}/keys`, by account. */
export function readDirectory(v: unknown): ReadonlyMap<string, DirectoryKey> | null {
  if (!isRecord(v) || !Array.isArray(v["keys"])) {
    return null;
  }
  const entries = v["keys"].map((entry: unknown): readonly [string, DirectoryKey] | null => {
    if (!isRecord(entry) || !isId(entry["account"]) || !isId(entry["key_id"]) || typeof entry["agreement_public"] !== "string") {
      return null;
    }
    const agreementPublic = decode(entry["agreement_public"]);
    return agreementPublic !== null && agreementPublic.length === 32 ? [entry["account"], { keyId: entry["key_id"], agreementPublic }] : null;
  });
  return entries.every((entry) => entry !== null) ? new Map(entries) : null;
}

/**
 * The recipients a revision sealed by the key `own` wraps to: the author's key and every share's, but `own`, which the
 * revision's body carries. Null when a listed key is not the one the directory or the group holds under that id:
 * the record or a key moved, and the shares are read again.
 */
export function revisionRecipients(
  listed: ListedShares,
  directory: ReadonlyMap<string, DirectoryKey>,
  groups: ReadonlyMap<string, Bytes>,
  own: string,
): readonly ShareRecipient[] | null {
  const wanted: readonly ListedShare[] = [
    ...(listed.author.keyId === null ? [] : [{ kind: "member", target: listed.author.account, right: "edit", keyId: listed.author.keyId } as const]),
    ...listed.shares,
  ];
  const recipients: ShareRecipient[] = [];
  for (const share of wanted.filter((each) => !(each.kind === "member" && each.keyId === own))) {
    const keyId = keyIdBytes(share.keyId);
    const member = share.kind === "member" ? directory.get(share.target) : undefined;
    const key = share.kind === "member" ? (member?.keyId === share.keyId ? member.agreementPublic : undefined) : groups.get(share.target);
    if (keyId === null || key === undefined) {
      return null;
    }
    recipients.push({
      recipient: share.kind === "member" ? { kind: "member", account: share.target } : { kind: "group", group: share.target },
      keyId,
      listed: key,
    });
  }
  return recipients;
}
