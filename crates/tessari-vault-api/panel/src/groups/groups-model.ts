// The groups page's model: the company's departments and teams as the server lists them, put in reading order with
// each team under its department; the member keys waiting for a group's key; and the words for a refused change. No
// DOM here, so all of it is tested.

import { decode } from "../base64url.ts";
import { keyIdBytes } from "../unlock-model.ts";
import type { PendingKey } from "./group-key.ts";

const KEY_BYTES = 32;

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isId = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";

/** One department or team, with the group it sits in. */
export interface GroupRow {
  readonly id: string;
  readonly kind: "department" | "team";
  readonly name: string;
  readonly parent: string;
}

/** The answer of `GET /api/companies/{company}/groups`, with each group's parent. */
export function readGroupRows(v: unknown): readonly GroupRow[] | null {
  if (!isRecord(v) || !Array.isArray(v["groups"])) {
    return null;
  }
  const rows = v["groups"].map((group: unknown): GroupRow | null => {
    if (!isRecord(group) || !isId(group["id"]) || !isText(group["name"]) || !isId(group["parent"])) {
      return null;
    }
    const { kind } = group;
    return kind === "department" || kind === "team" ? { id: group["id"], kind, name: group["name"], parent: group["parent"] } : null;
  });
  return rows.every((row) => row !== null) ? rows : null;
}

/** A line of the page: the company, a department, or a team, with how deep it sits. */
export interface TreeRow {
  readonly id: string;
  readonly kind: "company" | "department" | "team";
  readonly name: string;
  readonly depth: number;
}

/** The company, then each department by name with its teams by name under it, then the company's own teams. */
export function groupTree(company: { readonly id: string; readonly name: string }, rows: readonly GroupRow[]): readonly TreeRow[] {
  const byName = (one: GroupRow, other: GroupRow): number => one.name.localeCompare(other.name);
  const teamsOf = (parent: string, depth: number): TreeRow[] =>
    rows
      .filter((row) => row.kind === "team" && row.parent === parent)
      .sort(byName)
      .map((row) => ({ id: row.id, kind: row.kind, name: row.name, depth }));
  const departments = rows
    .filter((row) => row.kind === "department")
    .sort(byName)
    .flatMap((row): TreeRow[] => [{ id: row.id, kind: row.kind, name: row.name, depth: 1 }, ...teamsOf(row.id, 2)]);
  return [{ id: company.id, kind: "company", name: company.name, depth: 0 }, ...departments, ...teamsOf(company.id, 1)];
}

/** The answer of `GET …/groups/{group}/key/pending`: member keys in the group's reach with no wrap of its key. */
export function readPending(v: unknown): readonly PendingKey[] | null {
  if (!isRecord(v) || !Array.isArray(v["pending"])) {
    return null;
  }
  const keys = v["pending"].map((item: unknown): PendingKey | null => {
    if (!isRecord(item) || !isId(item["account"]) || typeof item["key_id"] !== "string" || typeof item["agreement_public"] !== "string") {
      return null;
    }
    const keyId = keyIdBytes(item["key_id"]);
    const agreementPublic = decode(item["agreement_public"]);
    return keyId === null || agreementPublic === null || agreementPublic.length !== KEY_BYTES ? null : { account: item["account"], keyId, agreementPublic };
  });
  return keys.every((key) => key !== null) ? keys : null;
}

/** Why a group could not be made, renamed or deleted, from the server's code, or `fallback`. */
export function createFailure(code: string, fallback: string): string {
  switch (code) {
    case "wrong_parent":
      return "A department sits in the company; a team sits in a department or in the company.";
    case "forbidden":
      return "Only an owner or an admin can make departments and teams.";
    case "group_not_empty":
      return "This department still holds teams. Delete or move them first.";
    default:
      return fallback;
  }
}

/** Whether a group's key (`GET …/groups/{group}/key`) is due for a new one, as after a member left; null if unsaid. */
export function readRotationDue(v: unknown): boolean | null {
  return isRecord(v) && typeof v["rotation_due"] === "boolean" ? v["rotation_due"] : null;
}
