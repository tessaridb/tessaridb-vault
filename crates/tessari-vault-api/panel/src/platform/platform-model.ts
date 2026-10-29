// The platform admin's Companies page, without a DOM: the deletions owners asked for, each company with the deletion
// waiting on it, the answer to creating a company, and a refused step in words.

import type { Failure } from "../api.ts";
import { readStanding, type Standing } from "../company/company-model.ts";
import type { Company } from "../unlock-model.ts";

/** A deletion an owner asked for, by company. */
export interface Open {
  readonly company: string;
  readonly standing: Standing;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";

export function readQueue(v: unknown): readonly Open[] | null {
  if (!isRecord(v) || !Array.isArray(v["drops"])) {
    return null;
  }
  const open: Open[] = [];
  for (const item of v["drops"]) {
    const standing = readStanding(item);
    if (!isRecord(item) || !isText(item["company"]) || standing === null) {
      return null;
    }
    open.push({ company: item["company"], standing });
  }
  return open;
}

/** A company as the platform admin sees it. */
export interface Row {
  readonly id: string;
  readonly name: string;
  readonly state: "active" | "suspended" | "closing";
  readonly deletion: Standing | null;
}

export const platformRows = (companies: readonly Company[], open: readonly Open[]): readonly Row[] =>
  companies.map((company) => ({
    id: company.id,
    name: company.name,
    state: company.active ? "active" : company.closing ? "closing" : "suspended",
    deletion: open.find((each) => each.company === company.id)?.standing ?? null,
  }));

/** A company just created, with its owner's single-use join token (none when the owner already has a sign-in). */
export function readCreated(v: unknown): { readonly id: string; readonly name: string; readonly token: string | null } | null {
  if (!isRecord(v) || !isText(v["id"]) || !isText(v["name"])) {
    return null;
  }
  const token = v["owner_enrolment"];
  return token === null || isText(token) ? { id: v["id"], name: v["name"], token } : null;
}

export function platformFailure(failure: Failure): string {
  switch (failure.code) {
    case "invalid":
      return "Give the company a name, and its owner's work email and name.";
    case "forbidden":
      return "Only the platform admin manages companies.";
    case "not_found":
      return "No owner is asking for this deletion any more. The list is shown again.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    default:
      return "The vault couldn't make this change. Try again.";
  }
}
