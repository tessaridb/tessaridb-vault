// Pieces every view draws with: its heading, the trail above it, and a refusal turned into words.

import { el } from "./dom.ts";
import type { Crumb } from "./records/crumbs.ts";

/** The view's heading, focusable so a route change can move focus to it. */
export const heading = (title: string): HTMLElement => el("h1", { tabindex: "-1", class: "view-title" }, title);

/** An inline message for a refused step: what happened and what to do next. */
export const refusal = (words: string): HTMLElement => el("p", { class: "message bad", role: "alert" }, words);

/** The trail above a heading: each step a link back, the page itself marked as current; separators are drawn by CSS. */
export const breadcrumbs = (trail: readonly Crumb[]): HTMLElement =>
  el(
    "nav",
    { class: "crumbs", "aria-label": "Breadcrumb" },
    el("ol", {}, ...trail.map((crumb) => el("li", {}, crumb.href === null ? el("span", { "aria-current": "page" }, crumb.label) : el("a", { href: crumb.href }, crumb.label)))),
  );
