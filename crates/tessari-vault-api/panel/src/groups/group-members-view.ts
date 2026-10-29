// A department's or team's members in a dialog: who is in it, a way to add a member of the company, and a way to take
// someone out. Taking someone out retires the group's key, so the page asks for a new one when the dialog closes.

import { call, ignored } from "../api.ts";
import { el, fill } from "../dom.ts";
import { loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import { refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { addable, readGroupMembers } from "./group-members.ts";
import type { TreeRow } from "./groups-model.ts";

/** The "Members" button of group `row` and the dialog it opens; `changed` runs once the dialog closes after a change. */
export function membersControl(held: Unlocked, row: TreeRow, changed: () => void): readonly HTMLElement[] {
  const path = `${companyPath(held)}/groups/${encodeURIComponent(row.id)}/members`;
  const body = el("div", {});
  const said = el("div", { "aria-live": "polite" });
  const close = el("button", { type: "button" }, "Close");
  const dialog = el("dialog", { class: "confirm", "aria-labelledby": `members-${row.id}` }, el("h2", { id: `members-${row.id}` }, `People in “${row.name}”`), body, said, el("div", { class: "actions" }, close));
  let touched = false;
  const act = (method: "POST" | "DELETE", url: string, payload?: Readonly<Record<string, unknown>>): void => {
    fill(said, el("p", { class: "muted", role: "status" }, "Saving."));
    void call(method, url, ignored, payload === undefined ? {} : { body: payload }).then((answer) => {
      if (!answer.ok) {
        fill(said, refusal(answer.code === "not_a_member" ? "Only an active member of the company can join a group." : loadFailure(answer)));
        return;
      }
      touched = true;
      fill(said, null);
      void draw();
    });
  };
  const draw = async (): Promise<void> => {
    const [inGroup, company] = await Promise.all([call("GET", path, readGroupMembers), call("GET", `${companyPath(held)}/members`, readMemberList)]);
    if (!inGroup.ok || !company.ok) {
      const failed = !inGroup.ok ? inGroup : company;
      fill(body, failed.ok ? null : refusal(loadFailure(failed)));
      return;
    }
    const people = inGroup.value.map((member) => {
      const remove = el("button", { type: "button", "aria-label": `Take ${member.name} out of ${row.name}` }, "Take out");
      remove.addEventListener("click", () => act("DELETE", `${path}/${encodeURIComponent(member.account)}`));
      return el("li", { class: "share member" }, el("div", {}, el("span", { class: "share-name" }, member.name), el("span", { class: "detail" }, member.email)), el("span", {}), remove);
    });
    const candidates = addable(company.value, inGroup.value);
    let chosen: string | null = null;
    const choices = candidates.map((member) => {
      const id = `join-${row.id}-${member.account}`;
      const radio = el("input", { id, type: "radio", name: `join-${row.id}` });
      radio.addEventListener("change", () => (chosen = member.account));
      return el("div", { class: "choice" }, radio, el("label", { for: id }, member.name, el("span", { class: "detail" }, member.email)));
    });
    const add = el("button", { type: "button", class: "primary" }, "Add to group");
    add.addEventListener("click", () => (chosen === null ? fill(said, refusal("Choose who to add.")) : act("POST", path, { account_id: chosen })));
    fill(
      body,
      people.length === 0 ? el("p", { class: "muted" }, "Nobody is in this group yet.") : el("ul", { class: "shares" }, ...people),
      candidates.length === 0 ? null : el("fieldset", { class: "choices" }, el("legend", {}, "Add someone"), el("div", { class: "targets" }, ...choices)),
      candidates.length === 0 ? null : add,
    );
  };
  const open = el("button", { type: "button", "aria-label": `People in ${row.name}` }, "Members");
  open.addEventListener("click", () => {
    touched = false;
    fill(said, null);
    fill(body, el("p", { class: "muted", role: "status" }, "Loading who is in the group."));
    dialog.showModal();
    void draw();
  });
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => {
    if (touched) {
      changed();
    }
  });
  return [open, dialog];
}
