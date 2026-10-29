// The records list: every record this person can open in the unlocked company, opened here, filtered by title and
// type, a page at a time. A record that would not open stays in the list with the reason, never as a blank row.

import { call, type Failure } from "../api.ts";
import { el, fill } from "../dom.ts";
import { openedPagePath, readOpenedPage } from "../records-model.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import type { Stops } from "./field-view.ts";
import { findsRecords, inScope, matches, scopeGroups, typeName, unreadableWords, type Listed, type Scope } from "./listing.ts";
import { Opener, companyPath, vouchedChain } from "./load.ts";
import { readGroupList, type ListedGroup } from "./share-targets.ts";

const when = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export const loadFailure = (failure: Failure): string =>
  failure.code === "network"
    ? "The vault didn't answer. Check your connection, then try again."
    : "The vault couldn't load your records. Try again in a minute.";

/** How long the list loads before its skeleton shows. */
const SKELETON_AFTER_MS = 200;

export const CHAIN_REFUSED =
  "Your company's list of who may read what doesn't check out against the owner key your keys trust, so nothing is opened. Tell your company's owner.";

const sameScope = (a: Scope, b: Scope): boolean => a.kind === b.kind && (a.kind !== "group" || (b.kind === "group" && a.group === b.group));

function row(item: Listed, href: (record: string) => string): HTMLElement {
  const changed = el("span", { class: "muted" }, `Changed ${when.format(new Date(item.row.updated * 1000))}`);
  if (!("document" in item)) {
    return el(
      "li",
      { class: "record unreadable" },
      el("span", { class: "record-title" }, "Record not shown"),
      el("span", { class: "muted" }, typeName(item.row.recordType)),
      el("p", { class: "message bad" }, unreadableWords(item.problem)),
    );
  }
  return el(
    "li",
    { class: "record" },
    el("a", { class: "record-title", href: href(item.row.id) }, item.document.title === "" ? "Untitled" : item.document.title),
    el("span", { class: "muted" }, typeName(item.row.recordType)),
    changed,
  );
}

/** Draws the list into `main`; `href` names the view of one record. */
export async function drawList(main: HTMLElement, held: Unlocked, href: (record: string) => string, newHref: string, stops: Stops): Promise<void> {
  const title = heading("Records");
  const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Opening your records."));
  const head = el("div", { class: "page-head" }, title, el("a", { class: "button primary", href: newHref }, "New record"));
  fill(main, head, said);
  // A load that takes longer than a glance shows where the rows will be, so the page does not sit blank.
  const skeleton = el("ul", { class: "records skeleton", "aria-hidden": "true" }, ...[1, 2, 3].map(() => el("li", { class: "record" }, el("span", { class: "bone" }))));
  const slow = window.setTimeout(() => main.append(skeleton), SKELETON_AFTER_MS);
  const loaded = (): void => {
    window.clearTimeout(slow);
    skeleton.remove();
  };
  stops.push(loaded);
  const [chain, listedGroups] = await Promise.all([vouchedChain(held), call("GET", `${companyPath(held)}/groups`, readGroupList)]);
  // Without the groups' names the list still shows every record; only the group chips are left out.
  const groups: readonly ListedGroup[] = listedGroups.ok ? listedGroups.value : [];
  if (!chain.ok) {
    loaded();
    fill(said, refusal("problem" in chain ? CHAIN_REFUSED : loadFailure(chain)));
    return;
  }
  const opener = new Opener(held, chain.vouched);
  const items: Listed[] = [];
  let next: string | null = null;
  let type: string | null = null;
  let scope: Scope = { kind: "all" };
  const search = el("input", { id: "record-filter", type: "search", autocomplete: "off", "aria-keyshortcuts": "Control+K Meta+K" });
  const toFilter = (event: KeyboardEvent): void => {
    if (findsRecords(event) && !filter.hidden) {
      event.preventDefault();
      search.focus();
    }
  };
  document.addEventListener("keydown", toFilter);
  stops.push(() => document.removeEventListener("keydown", toFilter));
  const scopes = el("div", { class: "chips", role: "group", "aria-label": "Show" });
  const types = el("div", { class: "chips", role: "group", "aria-label": "Type" });
  const list = el("ul", { class: "records", "aria-label": "Records" });
  const more = el("button", { type: "button", class: "quiet" }, "Load more");
  const tail = el("div", {});
  const filter = el("div", { class: "field" }, el("label", { for: "record-filter" }, "Find a record by title"), search);
  const render = (): void => {
    filter.hidden = items.length === 0;
    types.hidden = items.length === 0;
    scopes.hidden = items.length === 0;
    const offered: readonly (readonly [Scope, string])[] = [
      [{ kind: "all" }, "All"],
      [{ kind: "mine" }, "Mine"],
      ...scopeGroups(items, groups).map((group) => [{ kind: "group", group: group.id }, group.name] as const),
    ];
    fill(
      scopes,
      ...offered.map(([each, words]) => {
        const chip = el("button", { type: "button", class: "chip", "aria-pressed": String(sameScope(each, scope)) }, words);
        chip.addEventListener("click", () => {
          scope = each;
          render();
        });
        return chip;
      }),
    );
    const present = [...new Set(items.map((item) => item.row.recordType))];
    fill(
      types,
      ...[null, ...present].map((id) => {
        const chip = el("button", { type: "button", class: "chip", "aria-pressed": String(type === id) }, id === null ? "All types" : typeName(id));
        chip.addEventListener("click", () => {
          type = id;
          render();
        });
        return chip;
      }),
    );
    const kept = items.filter((item) => inScope(item, scope, held.account) && matches(item, search.value, type));
    fill(list, ...kept.map((item) => row(item, href)));
    list.hidden = kept.length === 0;
    if (items.length === 0) {
      fill(said, el("p", {}, "No records yet. Records you make, and records others share with you, appear here."));
    } else if (kept.length === 0) {
      const clear = el("button", { type: "button", class: "quiet" }, "Clear the filter");
      clear.addEventListener("click", () => {
        search.value = "";
        type = null;
        scope = { kind: "all" };
        render();
        search.focus();
      });
      const query = search.value.trim();
      fill(said, el("p", {}, query === "" ? "No records match these filters." : `No records match "${query}".`), clear);
    } else {
      fill(said);
    }
    fill(tail, next === null ? null : more);
  };
  const load = async (): Promise<boolean> => {
    more.disabled = true;
    const page = await call("GET", openedPagePath(held.company, next), readOpenedPage);
    more.disabled = false;
    if (!page.ok) {
      fill(said, refusal(loadFailure(page)));
      return false;
    }
    for (const opened of page.value.records) {
      items.push(await opener.open(opened));
    }
    next = page.value.next;
    return true;
  };
  const first = await load();
  loaded();
  if (!first) {
    return;
  }
  search.addEventListener("input", render);
  more.addEventListener("click", () => void load().then((loaded) => loaded && render()));
  fill(main, head, filter, scopes, types, said, list, tail);
  render();
  title.focus();
}
