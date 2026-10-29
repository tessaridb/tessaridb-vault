// The company trail, for owners and admins: every event newest first with who did what to whom and why, a filter by
// area, older events a page at a time, and a check that walks the whole chain and says whether any event was changed.

import { call } from "../api.ts";
import { el, fill } from "../dom.ts";
import { readGroupRows } from "../groups/groups-model.ts";
import { loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { actionWords, areaOf, pageFull, readEvents, readVerified, targetWords, verifyWords, whoWords, type Area, type TrailEvent } from "./trail-model.ts";

const AREAS: ReadonlyArray<readonly [Area | "all", string]> = [
  ["all", "All"],
  ["records", "Records"],
  ["members", "Members"],
  ["groups", "Groups"],
  ["keys", "Keys"],
  ["company", "Company"],
];

const when = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZoneName: "short" });

/** Draws the trail into `main`. */
export async function drawTrail(main: HTMLElement, held: Unlocked): Promise<void> {
  const title = heading("Trail");
  const head = el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, `Everything done in ${held.companyName}, newest first. Each event is written before the change it records.`)));
  fill(main, head, el("p", { class: "muted", role: "status" }, "Loading the trail."));
  const path = companyPath(held);
  const [first, members, groups] = await Promise.all([call("GET", `${path}/audit`, readEvents), call("GET", `${path}/members`, readMemberList), call("GET", `${path}/groups`, readGroupRows)]);
  if (!first.ok) {
    fill(main, head, refusal(first.status === 403 ? "Only the company's owners and admins can read the trail." : loadFailure(first)));
    return;
  }
  const names = new Map<string, string>([[held.company, held.companyName]]);
  for (const member of members.ok ? members.value : []) {
    names.set(member.account, member.name);
  }
  for (const group of groups.ok ? groups.value : []) {
    names.set(group.id, group.name);
  }

  const events: TrailEvent[] = [...first.value];
  let area: Area | "all" = "all";
  const body = el("tbody", {});
  const said = el("div", { "aria-live": "polite" });
  const older = el("button", { type: "button" }, "Older events");
  const draw = (): void => {
    const shown = events.filter((event) => area === "all" || areaOf(event.action) === area);
    fill(
      body,
      ...shown.map((event) =>
        el(
          "tr",
          {},
          el("td", { class: "when" }, when.format(new Date(event.at * 1000))),
          el("td", {}, whoWords(event.actor, names)),
          el("td", {}, actionWords(event.action)),
          el("td", {}, targetWords(event.target, names)),
          el("td", { class: "muted" }, event.reason),
        ),
      ),
    );
    if (shown.length === 0) {
      fill(body, el("tr", {}, el("td", { colspan: "5", class: "muted" }, "Nothing of this kind in the events read so far.")));
    }
  };
  older.hidden = !pageFull(first.value.length);
  older.addEventListener("click", () => {
    older.disabled = true;
    void call("GET", `${path}/audit?before=${events.at(-1)?.seq ?? 0}`, readEvents).then((next) => {
      older.disabled = false;
      if (!next.ok) {
        fill(said, refusal(loadFailure(next)));
        return;
      }
      events.push(...next.value);
      older.hidden = !pageFull(next.value.length);
      draw();
    });
  });

  const chips = AREAS.map(([each, words]) => el("button", { type: "button", class: "chip", "aria-pressed": String(each === area) }, words));
  chips.forEach((chip, at) =>
    chip.addEventListener("click", () => {
      area = AREAS[at]?.[0] ?? "all";
      chips.forEach((other, index) => other.setAttribute("aria-pressed", String(index === at)));
      draw();
    }),
  );
  const check = el("button", { type: "button" }, "Check the trail");
  check.addEventListener("click", () => {
    check.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Checking every event."));
    void call("GET", `${path}/audit/verify`, readVerified).then((verified) => {
      check.disabled = false;
      fill(said, verified.ok ? (verified.value.intact ? el("p", { class: "muted", role: "status" }, verifyWords(verified.value)) : refusal(verifyWords(verified.value))) : refusal(loadFailure(verified)));
    });
  });

  draw();
  fill(
    main,
    head,
    el("div", { class: "toolbar" }, el("div", { class: "chips", role: "group", "aria-label": "Show" }, ...chips), check),
    said,
    el(
      "div",
      { class: "table-frame", tabindex: "0", role: "region", "aria-label": "Events" },
      el("table", { class: "records trail" }, el("thead", {}, el("tr", {}, ...["When", "Who", "What", "On", "Why"].map((words) => el("th", { scope: "col" }, words)))), body),
    ),
    older,
  );
  title.focus();
}
