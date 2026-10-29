// Adding someone to the company, for owners and admins: their name, work email and role, then the single-use join link
// the vault issues for them, which the page shows once for the admin to send. Leaving the page drops the link; a new
// one can be issued from the member's own page.

import { announce, el, fill } from "../dom.ts";
import { call } from "../api.ts";
import { companyPath } from "../records/load.ts";
import type { ListedMember } from "../records/share-targets.ts";
import { refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { joinLink, memberFailure, roleChoices } from "./member-admin.ts";

type Role = ListedMember["role"];

const ROLE_WORDS = { owner: "Owner", admin: "Admin", member: "Member" } as const;

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const readAdded = (v: unknown): string | null => (isRecord(v) && typeof v["account_id"] === "string" && /^[0-9a-f]{32}$/.test(v["account_id"]) ? v["account_id"] : null);
const readToken = (v: unknown): string | null => (isRecord(v) && typeof v["token"] === "string" && v["token"] !== "" ? v["token"] : null);

/** Who was just added and the link to send them, shown once at the top of the members page. */
export interface Invited {
  readonly name: string;
  readonly link: string;
}

/** The block showing `invited`'s join link with a way to copy it. */
export function invitedBlock(invited: Invited): HTMLElement {
  const copy = el("button", { type: "button" }, "Copy link");
  copy.addEventListener("click", () => {
    void navigator.clipboard.writeText(invited.link).then(
      () => announce("The join link is copied."),
      () => announce("The link couldn't be copied. Select it and copy it by hand."),
    );
  });
  return el(
    "section",
    { class: "record-section", "aria-labelledby": "invited" },
    el("h2", { id: "invited" }, `Send ${invited.name} this link`),
    el("p", { class: "muted" }, "It works once, within 24 hours. This page shows it only now."),
    el("p", { class: "mono" }, invited.link),
    copy,
  );
}

/** The form adding someone with a role `mine` may give; `added` runs with the link to show once they are in. */
export function addMemberForm(held: Unlocked, mine: Role, added: (invited: Invited) => void): HTMLElement {
  let role: Role = "member";
  const name = el("input", { id: "member-name", type: "text", autocomplete: "off" });
  const email = el("input", { id: "member-email", type: "email", autocomplete: "off", spellcheck: "false" });
  const choices = roleChoices(mine);
  const roles = choices.map((each) => el("button", { type: "button", class: "chip", "aria-pressed": String(each === role) }, ROLE_WORDS[each]));
  roles.forEach((button, at) =>
    button.addEventListener("click", () => {
      role = choices[at] ?? "member";
      roles.forEach((each, other) => each.setAttribute("aria-pressed", String(other === at)));
    }),
  );
  const submit = el("button", { type: "submit", class: "primary" }, "Add member");
  const said = el("div", { "aria-live": "polite" });
  const form = el(
    "form",
    { class: "record-section", novalidate: "" },
    el("h2", {}, "Add someone"),
    el("div", { class: "field" }, el("label", { for: name.id }, "Name"), name),
    el("div", { class: "field" }, el("label", { for: email.id }, "Work email"), email),
    el("div", { class: "field" }, el("p", { class: "field-label", id: "member-role" }, "Role"), el("div", { class: "chips", role: "group", "aria-labelledby": "member-role" }, ...roles)),
    submit,
    said,
  );
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    if (name.value.trim() === "" || email.value.trim() === "") {
      fill(said, refusal(memberFailure({ ok: false, status: 400, code: "invalid", message: "" })));
      (name.value.trim() === "" ? name : email).focus();
      return;
    }
    submit.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Adding them."));
    const path = `${companyPath(held)}/members`;
    void (async () => {
      const made = await call("POST", path, readAdded, { body: { name: name.value.trim(), email: email.value.trim(), role } });
      const link = made.ok ? await call("POST", `${path}/${made.value}/enrolment`, readToken) : made;
      submit.disabled = false;
      if (!link.ok) {
        fill(said, refusal(memberFailure(link)));
        return;
      }
      added({ name: name.value.trim(), link: joinLink(window.location.origin, window.location.pathname, link.value) });
    })();
  });
  return form;
}
