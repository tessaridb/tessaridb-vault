// A member's own page, for the owners and admins who may manage them: their role, suspending or resuming them, a new
// join link while they have not joined, and removing them, which is asked again in a dialog naming what changes. An
// admin sees an owner's page without the controls: an owner's acts are an owner's.

import { call, ignored } from "../api.ts";
import { el, fill } from "../dom.ts";
import { loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList, type ListedMember } from "../records/share-targets.ts";
import { breadcrumbs, heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { invitedBlock } from "./add-member.ts";
import { joinLink, mayManage, memberCrumbs, memberFailure, removeWords, roleChoices } from "./member-admin.ts";

const ROLE_WORDS = { owner: "Owner", admin: "Admin", member: "Member" } as const;
const ROLE_AS = { owner: "an owner", admin: "an admin", member: "a member" } as const;

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const readToken = (v: unknown): string | null => (isRecord(v) && typeof v["token"] === "string" && v["token"] !== "" ? v["token"] : null);

/** Draws member `account` into `main`; `members` is the Members page's link and `gone` runs once they are removed. */
export async function drawMember(main: HTMLElement, held: Unlocked, account: string, members: string, gone: () => void, notice: string | null = null): Promise<void> {
  fill(main, breadcrumbs(memberCrumbs(members, null)), el("div", { class: "page-head" }, heading("Member")), el("p", { class: "muted", role: "status" }, "Loading the member."));
  const listed = await call("GET", `${companyPath(held)}/members`, readMemberList);
  const them = listed.ok ? listed.value.find((each) => each.account === account) : undefined;
  const me = listed.ok ? listed.value.find((each) => each.account === held.account) : undefined;
  if (!listed.ok || them === undefined || me === undefined) {
    const words = listed.ok ? memberFailure({ ok: false, status: 404, code: "not_found", message: "" }) : loadFailure(listed);
    fill(main, breadcrumbs(memberCrumbs(members, null)), el("div", { class: "page-head" }, heading("Member")), refusal(words));
    return;
  }
  const base = `${companyPath(held)}/members/${encodeURIComponent(account)}`;
  const said = el("div", { "aria-live": "polite" }, notice === null ? null : el("p", { class: "muted", role: "status" }, notice));
  const again = (words: string | null = null): void => void drawMember(main, held, account, members, gone, words);
  const send = (method: "PATCH" | "DELETE", body: Readonly<Record<string, unknown>> | undefined, then: () => void): void => {
    fill(said, el("p", { class: "muted", role: "status" }, "Saving."));
    void call(method, base, ignored, body === undefined ? {} : { body }).then((answer) => (answer.ok ? then() : fill(said, refusal(memberFailure(answer)))));
  };
  const head = el("div", { class: "page-head" }, el("div", {}, heading(them.name), el("p", { class: "lede" }, `${ROLE_WORDS[them.role]} · ${them.email}${them.active ? "" : " · Suspended"}`)));
  if (!mayManage(me.role, them.role)) {
    fill(main, breadcrumbs(memberCrumbs(members, them.name)), head, refusal(memberFailure({ ok: false, status: 403, code: "forbidden", message: "" })));
    return;
  }
  fill(main, breadcrumbs(memberCrumbs(members, them.name)), head, said, roleSection(me, them, (role) => send("PATCH", { role }, () => again(`${them.name} is now ${ROLE_AS[role]}.`))), accessSection(them, send, again), linkSection(them, base, said), removeSection(held, them, send, gone));
}

function roleSection(me: ListedMember, them: ListedMember, change: (role: ListedMember["role"]) => void): HTMLElement {
  let role = them.role;
  const choices = roleChoices(me.role);
  const chips = choices.map((each) => el("button", { type: "button", class: "chip", "aria-pressed": String(each === role) }, ROLE_WORDS[each]));
  chips.forEach((chip, at) =>
    chip.addEventListener("click", () => {
      role = choices[at] ?? role;
      chips.forEach((each, other) => each.setAttribute("aria-pressed", String(other === at)));
    }),
  );
  const save = el("button", { type: "button" }, "Change role");
  save.addEventListener("click", () => (role === them.role ? undefined : change(role)));
  return el("section", { class: "record-section", "aria-labelledby": "member-role" }, el("h2", { id: "member-role" }, "Role"), el("div", { class: "chips", role: "group", "aria-labelledby": "member-role" }, ...chips), save);
}

function accessSection(them: ListedMember, send: (method: "PATCH", body: Readonly<Record<string, unknown>>, then: () => void) => void, again: (words: string) => void): HTMLElement {
  const button = el("button", { type: "button" }, them.active ? "Suspend" : "Resume");
  button.addEventListener("click", () =>
    send("PATCH", { state: them.active ? "suspended" : "active" }, () => again(them.active ? `${them.name} is suspended and can't sign in until resumed.` : `${them.name} can sign in again.`)),
  );
  const lead = them.active ? "Suspending stops them signing in and opening anything, until they are resumed." : "They can't sign in while suspended.";
  return el("section", { class: "record-section", "aria-labelledby": "member-access" }, el("h2", { id: "member-access" }, "Access"), el("p", { class: "muted" }, lead), button);
}

function linkSection(them: ListedMember, base: string, said: HTMLElement): HTMLElement {
  const shown = el("div", {});
  const button = el("button", { type: "button" }, "New join link");
  button.addEventListener("click", () => {
    button.disabled = true;
    void call("POST", `${base}/enrolment`, readToken).then((answer) => {
      button.disabled = false;
      if (!answer.ok) {
        fill(said, refusal(memberFailure(answer)));
        return;
      }
      fill(shown, invitedBlock({ name: them.name, link: joinLink(window.location.origin, window.location.pathname, answer.value) }));
    });
  });
  return el("section", { class: "record-section", "aria-labelledby": "member-link" }, el("h2", { id: "member-link" }, "Join link"), el("p", { class: "muted" }, "For someone who hasn't joined yet, or whose link expired."), button, shown);
}

function removeSection(held: Unlocked, them: ListedMember, send: (method: "DELETE", body: undefined, then: () => void) => void, gone: () => void): HTMLElement {
  const words = removeWords(them.name, held.companyName);
  const keep = el("button", { type: "button", autofocus: "" }, "Keep them");
  const confirmed = el("button", { type: "button", class: "danger" }, words.confirm);
  const dialog = el("dialog", { class: "confirm", "aria-labelledby": "remove-member-title" }, el("h2", { id: "remove-member-title" }, words.title), el("p", {}, words.body), el("div", { class: "actions" }, keep, confirmed));
  const remove = el("button", { type: "button", class: "danger" }, "Remove from the company");
  remove.addEventListener("click", () => dialog.showModal());
  keep.addEventListener("click", () => dialog.close());
  confirmed.addEventListener("click", () => {
    dialog.close();
    send("DELETE", undefined, gone);
  });
  return el("section", { class: "record-section", "aria-labelledby": "member-remove" }, el("h2", { id: "member-remove" }, "Remove"), remove, dialog);
}
