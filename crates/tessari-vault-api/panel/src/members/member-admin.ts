// Adding and managing a company's members besides the pages: the join link an admin hands on, the roles each role may
// give (an owner's acts are an owner's: making, unmaking or removing an owner), and the words for each refused change.

import type { Failure } from "../api.ts";
import type { ListedMember } from "../records/share-targets.ts";

type Role = ListedMember["role"];

/** The link to this vault's join page carrying `token`, for the admin to send; `path` is the page's own path. */
export const joinLink = (origin: string, path: string, token: string): string => `${origin}${path}#/join?token=${encodeURIComponent(token)}`;

/** The roles someone holding `mine` may give, in the order the page offers them. */
export function roleChoices(mine: Role): readonly Role[] {
  switch (mine) {
    case "owner":
      return ["member", "admin", "owner"];
    case "admin":
      return ["member", "admin"];
    case "member":
      return [];
  }
}

/** What went wrong and what to do, for a refused add, change, removal or join link. */
export function memberFailure(failure: Failure): string {
  switch (failure.code) {
    case "already_member":
      return "Someone with that email is already in the company.";
    case "last_owner":
      return "A company keeps at least one owner. Make someone else an owner first.";
    case "enrolled":
      return "They have already joined, so they don't need a join link. They sign in with their own password.";
    case "forbidden":
      return "Only an owner can make, change or remove an owner.";
    case "not_found":
      return "They aren't in the company any more. The list is shown again.";
    case "invalid":
      return "Give a name and a work email address.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    default:
      return "The vault couldn't make this change. Try again; if it keeps failing, tell your company's owner.";
  }
}

/** Whether someone holding `mine` may change, suspend or remove a member holding `theirs`: an owner's acts are an owner's. */
export const mayManage = (mine: Role, theirs: Role): boolean => mine === "owner" || (mine === "admin" && theirs !== "owner");

/** The trail above a member's own page; `name` is null until the member is read. */
export const memberCrumbs = (members: string, name: string | null): Array<{ readonly label: string; readonly href: string | null }> => [
  { label: "Members", href: members },
  { label: name ?? "Member", href: null },
];

/** What the dialog removing `name` from `company` says, and its confirm button. */
export function removeWords(name: string, company: string): { readonly title: string; readonly body: string; readonly confirm: string } {
  return {
    title: `Remove \u201C${name}\u201D from ${company}?`,
    body: "They lose every record and group at their next request. The keys they held are marked for new keys: make the groups\u2019 on the Groups page; each record gets one the next time it is saved.",
    confirm: `Remove ${name}`,
  };
}
