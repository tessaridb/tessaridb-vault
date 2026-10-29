// Which groups a record can be for: the whole company, its departments and its teams. The company is a group too,
// though the groups list leaves it out, so it is added ahead of the others. No DOM here, so all of it is tested.

import type { Bytes } from "../crypto/bytes.ts";
import { shareTargets, type ListedGroup, type ShareTarget } from "./share-targets.ts";

const ORDER: Readonly<Record<ListedGroup["kind"], number>> = { company: 0, department: 1, team: 2 };

/** The company as a group, then its departments, then its teams, each by name. */
export function companyAndGroups(company: { readonly id: string; readonly name: string }, groups: readonly ListedGroup[]): readonly ListedGroup[] {
  const itself: ListedGroup = { id: company.id, kind: "company", name: company.name };
  return [itself, ...groups.filter((group) => group.id !== company.id)].sort(
    (one, other) => ORDER[one.kind] - ORDER[other.kind] || one.name.localeCompare(other.name),
  );
}

/** The groups a new record can be for, each with the key a share to it wraps to, or why it has none yet. */
export function audienceChoices(
  company: { readonly id: string; readonly name: string },
  groups: readonly ListedGroup[],
  groupKeys: ReadonlyMap<string, { readonly keyId: string; readonly public: Bytes }>,
): readonly ShareTarget[] {
  return shareTargets([], companyAndGroups(company, groups), new Map(), groupKeys, "", []);
}
