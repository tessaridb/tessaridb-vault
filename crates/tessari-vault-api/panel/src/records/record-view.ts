// One record, opened here: its title, type and sections, each field with the actions it offers. Opening it is a read
// on the company's trail, the same as the list's.

import { call } from "../api.ts";
import { el, fill } from "../dom.ts";
import { readOpened } from "../records-model.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { fieldView, type Stops } from "./field-view.ts";
import { CHAIN_REFUSED, loadFailure } from "./list-view.ts";
import { typeName, unreadableWords, type copier } from "./listing.ts";
import { Opener, companyPath, vouchedChain } from "./load.ts";

/** Draws record `id` into `main`; `back` is the list's link. */
export async function drawRecord(
  main: HTMLElement,
  held: Unlocked,
  id: string,
  copy: ReturnType<typeof copier>,
  stops: Stops,
  back: string,
  editHref: string,
): Promise<void> {
  const toList = el("a", { href: back, class: "back" }, "All records");
  const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Opening the record."));
  fill(main, toList, el("div", { class: "page-head" }, heading("Record")), said);
  const [chain, opened] = await Promise.all([
    vouchedChain(held),
    call("GET", `${companyPath(held)}/records/${encodeURIComponent(id)}`, readOpened),
  ]);
  if (!chain.ok) {
    fill(said, refusal("problem" in chain ? CHAIN_REFUSED : loadFailure(chain)));
    return;
  }
  if (!opened.ok) {
    fill(said, refusal(opened.status === 404 ? "This record doesn't exist, or it isn't shared with you." : loadFailure(opened)));
    return;
  }
  const item = await new Opener(held, chain.vouched).open(opened.value);
  if (!("document" in item)) {
    fill(main, toList, el("div", { class: "page-head" }, heading("Record not shown")), refusal(unreadableWords(item.problem)));
    return;
  }
  const { document } = item;
  const title = heading(document.title === "" ? "Untitled" : document.title);
  const sections = document.sections.map((section) =>
    el(
      "section",
      { class: "record-section" },
      section.name === "" ? null : el("h2", {}, section.name),
      el("dl", { class: "fields" }, ...section.fields.flatMap((field) => fieldView(field, copy, stops))),
    ),
  );
  const head = el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, typeName(document.type))));
  const actions = el("a", { class: "button", href: editHref }, "Edit");
  fill(main, toList, el("div", { class: "page-head" }, head, actions), ...sections);
  title.focus();
}
