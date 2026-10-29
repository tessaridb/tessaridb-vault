// What the record page needs to show a record's sharing to an editor: its shares, named from the company's members and
// groups, and whom it can still be shared with, each with the key a share wraps to. One fetch per group key.

import { call, type Failure } from "../api.ts";
import { hex, type Bytes } from "../crypto/bytes.ts";
import { readGroupKey } from "../records-model.ts";
import type { Unlocked } from "../vault.ts";
import { loadFailure } from "./list-view.ts";
import { companyPath } from "./load.ts";
import { companyAndGroups } from "./audience.ts";
import { readGroupList, readMemberList, sharedRows, shareTargets, type ListedGroup, type SharedRow, type ShareTarget } from "./share-targets.ts";
import { readDirectory, readShares } from "./shares-model.ts";

export type Sharing =
  | { readonly ok: true; readonly rows: readonly SharedRow[]; readonly targets: readonly ShareTarget[] }
  | { readonly ok: false; readonly words: string };

const failed = (answer: Failure): Sharing => ({ ok: false, words: loadFailure(answer) });

/** Record `id`'s sharing as its editors see it; null when the person may read the record but not change its sharing. */
export async function loadSharing(held: Unlocked, id: string): Promise<Sharing | null> {
  const path = companyPath(held);
  const [listed, members, groups, directory] = await Promise.all([
    call("GET", `${path}/records/${encodeURIComponent(id)}/shares`, readShares),
    call("GET", `${path}/members`, readMemberList),
    call("GET", `${path}/groups`, readGroupList),
    call("GET", `${path}/keys`, readDirectory),
  ]);
  if (!listed.ok) {
    return listed.status === 403 ? null : failed(listed);
  }
  if (!members.ok) {
    return failed(members);
  }
  if (!groups.ok) {
    return failed(groups);
  }
  if (!directory.ok) {
    return failed(directory);
  }
  const all = companyAndGroups({ id: held.company, name: held.companyName }, groups.value);
  const groupKeys = await loadGroupKeys(held, all);
  if (!groupKeys.ok) {
    return failed(groupKeys);
  }
  const { author, shares } = listed.value;
  return {
    ok: true,
    rows: sharedRows(shares, members.value, all),
    targets: shareTargets(members.value, all, directory.value, groupKeys.keys, author.account, shares.map((share) => share.target)),
  };
}

/** The key each of `groups` has, by group; a group with none yet is left out. */
export async function loadGroupKeys(
  held: Unlocked,
  groups: readonly ListedGroup[],
): Promise<{ readonly ok: true; readonly keys: ReadonlyMap<string, { readonly keyId: string; readonly public: Bytes }> } | Failure> {
  const path = companyPath(held);
  const answers = await Promise.all(
    groups.map((group) => call("GET", `${path}/groups/${encodeURIComponent(group.id)}/key`, readGroupKey).then((key) => [group.id, key] as const)),
  );
  const keys = new Map<string, { readonly keyId: string; readonly public: Bytes }>();
  for (const [group, key] of answers) {
    if (key.ok) {
      keys.set(group, { keyId: hex(key.value.keyId), public: key.value.public });
    } else if (key.status !== 404) {
      return key;
    }
  }
  return { ok: true, keys };
}
