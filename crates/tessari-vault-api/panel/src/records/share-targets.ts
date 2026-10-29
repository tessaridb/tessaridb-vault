// Whom a record can be shared with from its page: the company's active members and its groups, without its author or
// a target it is shared with already. Each target carries the public key its share wraps to, or the words for why it
// has none yet. No DOM here, so all of it is tested; every key still passes the chain check when the share is sealed.

import type { Bytes } from "../crypto/bytes.ts";
import { keyIdBytes } from "../unlock-model.ts";
import type { ShareRecipient } from "./save-record.ts";
import type { DirectoryKey, ListedShare } from "./shares-model.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isId = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";

export interface ListedMember {
  readonly account: string;
  readonly name: string;
  readonly email: string;
  readonly role: "owner" | "admin" | "member";
  readonly active: boolean;
}

/** The answer of `GET /api/companies/{company}/members`. */
export function readMemberList(v: unknown): readonly ListedMember[] | null {
  if (!isRecord(v) || !Array.isArray(v["members"])) {
    return null;
  }
  const members = v["members"].map((member: unknown): ListedMember | null => {
    if (!isRecord(member) || !isRecord(member["account"]) || typeof member["state"] !== "string") {
      return null;
    }
    const { id, name, email } = member["account"];
    const { role } = member;
    return isId(id) && isText(name) && isText(email) && (role === "owner" || role === "admin" || role === "member")
      ? { account: id, name, email, role, active: member["state"] === "active" }
      : null;
  });
  return members.every((member) => member !== null) ? members : null;
}

export interface ListedGroup {
  readonly id: string;
  readonly kind: "company" | "department" | "team";
  readonly name: string;
}

/** The answer of `GET /api/companies/{company}/groups`. */
export function readGroupList(v: unknown): readonly ListedGroup[] | null {
  if (!isRecord(v) || !Array.isArray(v["groups"])) {
    return null;
  }
  const groups = v["groups"].map((group: unknown): ListedGroup | null => {
    if (!isRecord(group) || !isId(group["id"]) || !isText(group["name"])) {
      return null;
    }
    const { kind } = group;
    return kind === "company" || kind === "department" || kind === "team" ? { id: group["id"], kind, name: group["name"] } : null;
  });
  return groups.every((group) => group !== null) ? groups : null;
}

/** One row of the share dialog: its words, and the recipient a share to it wraps to, or why there is none yet. */
export type ShareTarget =
  | { readonly id: string; readonly label: string; readonly detail: string; readonly recipient: ShareRecipient }
  | { readonly id: string; readonly label: string; readonly detail: string; readonly recipient: null; readonly missing: string };

/** The targets a record written by `author` and already shared with `shared` (target ids) can be shared with. */
export function shareTargets(
  members: readonly ListedMember[],
  groups: readonly ListedGroup[],
  directory: ReadonlyMap<string, DirectoryKey>,
  groupKeys: ReadonlyMap<string, { readonly keyId: string; readonly public: Bytes }>,
  author: string,
  shared: readonly string[],
): readonly ShareTarget[] {
  const open = (id: string): boolean => id !== author && !shared.includes(id);
  const people = members
    .filter((member) => member.active && open(member.account))
    .map((member): ShareTarget => {
      const listed = directory.get(member.account);
      const keyId = listed === undefined ? null : keyIdBytes(listed.keyId);
      return listed === undefined || keyId === null
        ? { id: member.account, label: member.name, detail: member.email, recipient: null, missing: "hasn't set an unlock password yet" }
        : { id: member.account, label: member.name, detail: member.email, recipient: { recipient: { kind: "member", account: member.account }, keyId, listed: listed.agreementPublic } };
    });
  const teams = groups
    .filter((group) => open(group.id))
    .map((group): ShareTarget => {
      const listed = groupKeys.get(group.id);
      const keyId = listed === undefined ? null : keyIdBytes(listed.keyId);
      const detail = groupDetail(group.kind);
      return listed === undefined || keyId === null
        ? { id: group.id, label: group.name, detail, recipient: null, missing: "has no key yet" }
        : { id: group.id, label: group.name, detail, recipient: { recipient: { kind: "group", group: group.id }, keyId, listed: listed.public } };
    });
  return [...people, ...teams];
}

/** The targets whose name or detail holds `text`, ignoring case and the spaces around it. */
export function filterTargets(targets: readonly ShareTarget[], text: string): readonly ShareTarget[] {
  const wanted = text.trim().toLowerCase();
  return targets.filter((target) => `${target.label}\n${target.detail}`.toLowerCase().includes(wanted));
}

/** One row of a record's "Shared with" list. */
export interface SharedRow {
  readonly target: string;
  readonly label: string;
  readonly detail: string;
  readonly right: "read" | "edit";
}

/** Each of `shares` named from the company's members and groups; one neither lists any more is said to be so. */
export function sharedRows(shares: readonly ListedShare[], members: readonly ListedMember[], groups: readonly ListedGroup[]): readonly SharedRow[] {
  return shares.map((share): SharedRow => {
    const { target, right } = share;
    if (share.kind === "member") {
      const member = members.find((each) => each.account === target);
      return member === undefined
        ? { target, label: "A member no longer listed", detail: "", right }
        : { target, label: member.name, detail: member.email, right };
    }
    const group = groups.find((each) => each.id === target);
    return group === undefined ? { target, label: "A group no longer listed", detail: "", right } : { target, label: group.name, detail: groupDetail(group.kind), right };
  });
}

const groupDetail = (kind: ListedGroup["kind"]): string => (kind === "company" ? "Everyone in the company" : kind === "department" ? "Department" : "Team");
