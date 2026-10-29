// Who is in a department or team, as the server lists them, and whom an admin can add to it. No DOM here, so all of
// it is tested.

import type { ListedMember } from "../records/share-targets.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isId = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";

/** One person in a group. */
export interface GroupMember {
  readonly account: string;
  readonly email: string;
  readonly name: string;
}

/** The answer of `GET /api/companies/{company}/groups/{group}/members`. */
export function readGroupMembers(v: unknown): readonly GroupMember[] | null {
  if (!isRecord(v) || !Array.isArray(v["members"])) {
    return null;
  }
  const members = v["members"].map((member: unknown): GroupMember | null =>
    isRecord(member) && isId(member["id"]) && isText(member["email"]) && isText(member["name"]) ? { account: member["id"], email: member["email"], name: member["name"] } : null,
  );
  return members.every((member) => member !== null) ? members : null;
}

/** The company's active members who are not in the group yet, by name. */
export function addable(company: readonly ListedMember[], inGroup: readonly GroupMember[]): readonly ListedMember[] {
  const inside = new Set(inGroup.map((member) => member.account));
  return company.filter((member) => member.active && !inside.has(member.account)).sort((one, other) => one.name.localeCompare(other.name));
}
