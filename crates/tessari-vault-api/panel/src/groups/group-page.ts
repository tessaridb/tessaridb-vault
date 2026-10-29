// A department's or team's own page: who is in it, a way to add a member of the company and to take someone out, its
// name, and deleting it, which is asked again in a dialog because records shared with the group stop opening for the
// people who reached them through it. Taking someone out retires the group's key; the Groups page then offers a new one.

import { call, ignored } from "../api.ts";
import { el, fill } from "../dom.ts";
import { loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import { breadcrumbs, heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { addable, readGroupMembers } from "./group-members.ts";
import { groupCrumbs, takeOutButton, type ButtonWords } from "./group-words.ts";
import { createFailure, readGroupRows } from "./groups-model.ts";

const KIND_WORDS = { department: "Department", team: "Team" } as const;

/** A button showing `words.text`, named `words.label`. */
export const labelled = (words: ButtonWords): HTMLButtonElement => el("button", { type: "button", "aria-label": words.label }, words.text);

/** Draws group `id` into `main`; `groups` is the Groups page's link and `gone` runs once the group is deleted. */
export async function drawGroup(main: HTMLElement, held: Unlocked, id: string, groups: string, gone: () => void, notice: string | null = null): Promise<void> {
  const base = `${companyPath(held)}/groups/${encodeURIComponent(id)}`;
  fill(main, breadcrumbs(groupCrumbs(groups, null)), el("div", { class: "page-head" }, heading("Group")), el("p", { class: "muted", role: "status" }, "Loading the group."));
  const [listed, inGroup, company] = await Promise.all([
    call("GET", `${companyPath(held)}/groups`, readGroupRows),
    call("GET", `${base}/members`, readGroupMembers),
    call("GET", `${companyPath(held)}/members`, readMemberList),
  ]);
  const failed = [listed, inGroup, company].find((answer) => !answer.ok);
  const row = listed.ok ? listed.value.find((each) => each.id === id) : undefined;
  if (failed !== undefined || !listed.ok || !inGroup.ok || !company.ok || row === undefined) {
    const words = failed !== undefined && !failed.ok ? loadFailure(failed) : "This group doesn't exist any more.";
    fill(main, breadcrumbs(groupCrumbs(groups, null)), el("div", { class: "page-head" }, heading("Group")), refusal(words));
    return;
  }
  const said = el("div", { "aria-live": "polite" }, notice === null ? null : el("p", { class: "muted", role: "status" }, notice));
  const again = (words: string | null = null): void => void drawGroup(main, held, id, groups, gone, words);
  const send = (method: "POST" | "PATCH" | "DELETE", url: string, payload: Readonly<Record<string, unknown>> | undefined, then: () => void): void => {
    fill(said, el("p", { class: "muted", role: "status" }, "Saving."));
    void call(method, url, ignored, payload === undefined ? {} : { body: payload }).then((answer) => {
      if (!answer.ok) {
        fill(said, refusal(answer.code === "not_a_member" ? "Only an active member of the company can join a group." : createFailure(answer.code, loadFailure(answer))));
        return;
      }
      then();
    });
  };

  const people = inGroup.value.map((member) => {
    const remove = labelled(takeOutButton(member.name, row.name));
    remove.addEventListener("click", () => send("DELETE", `${base}/members/${encodeURIComponent(member.account)}`, undefined, () => again(`${member.name} is out of ${row.name}. The group needs a new key; make it on the Groups page.`)));
    return el("li", { class: "share" }, el("div", {}, el("span", { class: "share-name" }, member.name), el("span", { class: "detail" }, member.email)), el("span", {}), remove);
  });
  const candidates = addable(company.value, inGroup.value);
  let chosen: string | null = null;
  const choices = candidates.map((member) => {
    const radio = el("input", { id: `join-${member.account}`, type: "radio", name: "join" });
    radio.addEventListener("change", () => (chosen = member.account));
    return el("div", { class: "choice" }, radio, el("label", { for: radio.id }, member.name, el("span", { class: "detail" }, member.email)));
  });
  const add = el("button", { type: "button", class: "primary" }, "Add to group");
  add.addEventListener("click", () => (chosen === null ? fill(said, refusal("Choose who to add.")) : send("POST", `${base}/members`, { account_id: chosen }, () => again())));

  const name = el("input", { id: "group-rename", type: "text", autocomplete: "off", value: row.name });
  const rename = el("button", { type: "button" }, "Rename");
  rename.addEventListener("click", () => {
    const wanted = name.value.trim();
    if (wanted !== "" && wanted !== row.name) {
      send("PATCH", base, { name: wanted }, () => again());
    }
  });

  const keep = el("button", { type: "button" }, "Keep it");
  const confirmed = el("button", { type: "button", class: "danger" }, `Delete “${row.name}”`);
  const confirm = el(
    "dialog",
    { class: "confirm", "aria-labelledby": "delete-group-title" },
    el("h2", { id: "delete-group-title" }, `Delete “${row.name}”?`),
    el("p", {}, "Records shared with this group stop opening for the people who reached them through it."),
    el("div", { class: "actions" }, keep, confirmed),
  );
  const remove = el("button", { type: "button", class: "danger" }, "Delete group");
  remove.addEventListener("click", () => confirm.showModal());
  keep.addEventListener("click", () => confirm.close());
  confirmed.addEventListener("click", () => {
    confirm.close();
    send("DELETE", base, undefined, gone);
  });

  fill(
    main,
    breadcrumbs(groupCrumbs(groups, row.name)),
    el("div", { class: "page-head" }, el("div", {}, heading(row.name), el("p", { class: "lede" }, KIND_WORDS[row.kind]))),
    said,
    el(
      "section",
      { class: "record-section", "aria-labelledby": "in-group" },
      el("h2", { id: "in-group" }, "People in it"),
      people.length === 0 ? el("p", { class: "muted" }, "Nobody is in this group yet.") : el("ul", { class: "shares" }, ...people),
      candidates.length === 0 ? null : el("fieldset", { class: "choices" }, el("legend", {}, "Add someone"), el("div", { class: "targets" }, ...choices)),
      candidates.length === 0 ? null : add,
    ),
    el("section", { class: "record-section", "aria-labelledby": "group-name" }, el("h2", { id: "group-name" }, "Name"), el("div", { class: "field" }, el("label", { for: name.id }, "Group name"), name), rename),
    el("section", { class: "record-section", "aria-labelledby": "group-delete" }, el("h2", { id: "group-delete" }, "Delete"), remove),
    confirm,
  );
}
