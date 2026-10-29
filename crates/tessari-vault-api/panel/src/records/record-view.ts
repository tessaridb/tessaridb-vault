// One record, opened here: its title, type and sections, each field with the actions it offers. Opening it is a read
// on the company's trail, the same as the list's.

import { call } from "../api.ts";
import { el, fill } from "../dom.ts";
import { icon } from "../icons.ts";
import { readOpened } from "../records-model.ts";
import { breadcrumbs, heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { recordCrumbs } from "./crumbs.ts";
import { fieldView, type Stops } from "./field-view.ts";
import { deleteControls } from "./delete-dialog.ts";
import { CHAIN_REFUSED, loadFailure } from "./list-view.ts";
import { typeName, unreadableWords, type copier } from "./listing.ts";
import { Opener, companyPath, vouchedChain } from "./load.ts";
import { shareControls } from "./share-dialog.ts";
import { sharedWithSection } from "./shared-with-view.ts";
import { loadSharing } from "./sharing.ts";

/** Draws record `id` into `main`; `back` is the list's link, the first step of the trail. */
export async function drawRecord(
  main: HTMLElement,
  held: Unlocked,
  id: string,
  copy: ReturnType<typeof copier>,
  stops: Stops,
  back: string,
  editHref: string,
  gone: () => void,
): Promise<void> {
  const unnamed = breadcrumbs(recordCrumbs(back, null));
  const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Opening the record."));
  fill(main, unnamed, el("div", { class: "page-head" }, heading("Record")), said);
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
  const opener = new Opener(held, chain.vouched);
  const [item, sharing] = await Promise.all([opener.open(opened.value), loadSharing(held, id)]);
  if (!("document" in item)) {
    fill(main, unnamed, el("div", { class: "page-head" }, heading("Record not shown")), refusal(unreadableWords(item.problem)));
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
  const head = el("div", {}, title, el("p", { class: "lede" }, typeName(document.type)));
  const named = document.title === "" ? "Untitled" : document.title;
  const [remove, dialog] = deleteControls(held, id, named, gone);
  const again = (): void => void drawRecord(main, held, id, copy, stops, back, editHref, gone);
  const context = { held, vouched: chain.vouched, opener, opened: opened.value, title: named, done: again };
  const [share, shareDialog] = sharing?.ok === true ? shareControls(context, sharing.targets) : [];
  const shared =
    sharing === null
      ? []
      : sharing.ok
        ? sharedWithSection(held, id, sharing.rows, again)
        : [el("section", { class: "record-section" }, el("h2", {}, "Shared with"), refusal(sharing.words))];
  const edit = el("a", { class: "button icon-action quiet", href: editHref, "aria-label": `Edit ${named}`, "data-tip": "Edit" }, icon("edit"));
  const actions = el("div", { class: "actions" }, edit, share ?? null, remove ?? null);
  fill(main, breadcrumbs(recordCrumbs(back, document.title)), el("div", { class: "page-head" }, head, actions), ...sections, ...shared, dialog ?? null, shareDialog ?? null);
  title.focus();
}
