// Pieces every view draws with: its heading, and a refusal turned into words.

import { el } from "./dom.ts";

/** The view's heading, focusable so a route change can move focus to it. */
export const heading = (title: string): HTMLElement => el("h1", { tabindex: "-1", class: "view-title" }, title);

/** An inline message for a refused step: what happened and what to do next. */
export const refusal = (words: string): HTMLElement => el("p", { class: "message bad", role: "alert" }, words);
