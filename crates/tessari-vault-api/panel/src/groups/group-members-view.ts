// A department's or team's settings in a dialog: its name, deleting it, who is in it, a way to add a member of the
// company, and a way to take someone out. Taking someone out retires the group's key, so the page asks for a new one
// when the dialog closes.

import { call, ignored } from "../api.ts";
import { el, fill } from "../dom.ts";
import { loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import { refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { addable, readGroupMembers } from "./group-members.ts";
import { membersButton, takeOutButton, type ButtonWords } from "./group-words.ts";
import { createFailure, type TreeRow } from "./groups-model.ts";

/** A button showing `words.text`, named `words.label`. */
export const labelled = (words: ButtonWords): HTMLButtonElement => el("button", { type: "button", "aria-label": words.label }, words.text);

/** The "Members" button of group `row` and the dialog it opens; `changed` runs once the dialog closes after a change. */
export function membersControl(held: Unlocked, row: TreeRow, changed: () => void): readonly HTMLElement[] {
  const path = `${companyPath(held)}/groups/${encodeURIComponent(row.id)}/members`;
  const body = el("div", {});
  const said = el("div", { "aria-live": "polite" });
  const close = el("button", { type: "button" }, "Close");
  const dialog = el("dialog", { class: "confirm", "aria-labelledby": `members-${row.id}` }, el("h2", { id: `members-${row.id}` }, `People in “${row.name}”`), body, said, el("div", { class: "actions" }, close));
  let touched = false;
  const send = (method: "POST" | "PATCH" | "DELETE", url: string, payload?: Readonly<Record<string, unknown>>, then: () => void = () => void draw()): void => {
    fill(said, el("p", { class: "muted", role: "status" }, "Saving."));
    void call(method, url, ignored, payload === undefined ? {} : { body: payload }).then((answer) => {
      if (!answer.ok) {
        fill(said, refusal(answer.code === "not_a_member" ? "Only an active member of the company can join a group." : createFailure(answer.code, loadFailure(answer))));
        return;
      }
      touched = true;
      fill(said, null);
      then();
    });
  };
  const act = (method: "POST" | "DELETE", url: string, payload?: Readonly<Record<string, unknown>>): void => send(method, url, payload);
  const draw = async (): Promise<void> => {
    const [inGroup, company] = await Promise.all([call("GET", path, readGroupMembers), call("GET", `${companyPath(held)}/members`, readMemberList)]);
    if (!inGroup.ok || !company.ok) {
      const failed = !inGroup.ok ? inGroup : company;
      fill(body, failed.ok ? null : refusal(loadFailure(failed)));
      return;
    }
    const people = inGroup.value.map((member) => {
      const remove = labelled(takeOutButton(member.name, row.name));
      remove.addEventListener("click", () => act("DELETE", `${path}/${encodeURIComponent(member.account)}`));
      return el("li", { class: "share" }, el("div", {}, el("span", { class: "share-name" }, member.name), el("span", { class: "detail" }, member.email)), el("span", {}), remove);
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
  const name = el("input", { id: `group-rename-${row.id}`, type: "text", autocomplete: "off", value: row.name });
  const rename = el("button", { type: "button" }, "Rename");
  rename.addEventListener("click", () => {
    const wanted = name.value.trim();
    if (wanted === "" || wanted === row.name) {
      return;
    }
    send("PATCH", `${companyPath(held)}/groups/${encodeURIComponent(row.id)}`, { name: wanted });
  });
  // Deleting is said twice: records shared with the group stop opening for the people who reached them through it.
  const remove = el("button", { type: "button", class: "danger" }, "Delete group");
  const sure = el("div", { class: "actions" });
  sure.hidden = true;
  const keep = el("button", { type: "button" }, "Keep it");
  const gone = el("button", { type: "button", class: "danger" }, `Delete \u201C${row.name}\u201D`);
  fill(sure, el("p", {}, "Records shared with this group stop opening for the people who reached them through it."), keep, gone);
  remove.addEventListener("click", () => (sure.hidden = false));
  keep.addEventListener("click", () => (sure.hidden = true));
  gone.addEventListener("click", () => send("DELETE", `${companyPath(held)}/groups/${encodeURIComponent(row.id)}`, undefined, () => dialog.close()));
  const settings = el("section", { class: "record-section" }, el("div", { class: "field" }, el("label", { for: name.id }, "Name"), name), rename, remove, sure);
  dialog.insertBefore(settings, said);
  const open = labelled(membersButton(row.name));
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
