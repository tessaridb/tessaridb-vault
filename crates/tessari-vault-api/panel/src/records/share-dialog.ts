// Sharing a record from its page: a native dialog listing the members and groups it can still be shared with, found by
// a filter, and what the share lets them do. The record key is wrapped here, to a key the company's chain vouches for,
// and only the wrap is sent; the server decides whether this person may share.

import { call, ignored } from "../api.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import type { Vouched } from "../crypto/statements.ts";
import { el, fill } from "../dom.ts";
import type { OpenedRecord } from "../records-model.ts";
import { refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { loadFailure } from "./list-view.ts";
import { companyPath, type Opener } from "./load.ts";
import { shareBody } from "./share-record.ts";
import { filterTargets, type ShareTarget } from "./share-targets.ts";
import { sealFailure, shareFailure } from "./share-words.ts";

/** The record being shared, as its page opened it, and what to do once a share is made. */
export interface ShareContext {
  readonly held: Unlocked;
  readonly vouched: Vouched;
  readonly opener: Opener;
  readonly opened: OpenedRecord;
  readonly title: string;
  readonly done: () => void;
}

const RIGHTS = [
  { right: "read", words: "Can view" },
  { right: "edit", words: "Can edit" },
] as const;

/** The "Share" button and the dialog it opens, offering `targets`. */
export function shareControls(context: ShareContext, targets: readonly ShareTarget[]): readonly HTMLElement[] {
  let chosen: string | null = null;
  let right: "read" | "edit" = "read";
  const filter = el("input", { id: "share-filter", type: "search", autocomplete: "off", "aria-controls": "share-targets" });
  const list = el("div", { id: "share-targets", class: "targets" });
  const said = el("div", { "aria-live": "polite" });
  const rights = RIGHTS.map((each) => el("button", { type: "button", class: "chip", "aria-pressed": String(each.right === right) }, each.words));
  const cancel = el("button", { type: "button" }, "Cancel");
  const confirm = el("button", { type: "button", class: "primary" }, "Share");

  const draw = (): void => {
    const shown = filterTargets(targets, filter.value);
    if (shown.length === 0) {
      fill(list, el("p", { class: "muted" }, targets.length === 0 ? "Everyone in the company can open this record already." : `No member or group matches \u201C${filter.value.trim()}\u201D.`));
      return;
    }
    fill(list, ...shown.map((target) => targetRow(target, target.id === chosen, () => (chosen = target.id))));
  };
  const press = (at: number): void => {
    right = RIGHTS[at]?.right ?? "read";
    rights.forEach((each, other) => each.setAttribute("aria-pressed", String(other === at)));
  };
  rights.forEach((button, at) => button.addEventListener("click", () => press(at)));
  filter.addEventListener("input", draw);

  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": "share-title" },
    el("h2", { id: "share-title" }, `Share \u201C${context.title}\u201D`),
    el("div", { class: "field" }, el("label", { for: filter.id }, "Find a member or group"), filter),
    el("fieldset", { class: "choices" }, el("legend", {}, "Share with"), list),
    el("fieldset", { class: "choices" }, el("legend", {}, "What they can do"), el("div", { class: "chips" }, ...rights)),
    said,
    el("div", { class: "actions" }, cancel, confirm),
  );
  const open = el("button", { type: "button" }, "Share");
  open.addEventListener("click", () => {
    chosen = null;
    press(0);
    filter.value = "";
    fill(said, null);
    draw();
    dialog.showModal();
  });
  cancel.addEventListener("click", () => dialog.close());
  confirm.addEventListener("click", () => {
    const target = targets.find((each) => each.id === chosen);
    if (target?.recipient == null) {
      fill(said, refusal("Choose a member or group to share with."));
      return;
    }
    confirm.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Sharing the record."));
    void share(context, target.recipient, right).then((words) => {
      confirm.disabled = false;
      if (words === null) {
        dialog.close();
        context.done();
        return;
      }
      fill(said, refusal(words));
    });
  });
  return [open, dialog];
}

/** One radio row: the target's name and detail, or, when it has no key to share with, why, and disabled. */
function targetRow(target: ShareTarget, checked: boolean, choose: () => void): HTMLElement {
  const id = `share-target-${target.id}`;
  const radio = el("input", { id, type: "radio", name: "share-target", value: target.id, ...(checked ? { checked: "" } : {}) });
  if (target.recipient === null) {
    radio.disabled = true;
  }
  radio.addEventListener("change", choose);
  const detail = target.recipient === null ? `${target.detail} \u00B7 ${target.missing}` : target.detail;
  return el("div", { class: "choice" }, radio, el("label", { for: id }, target.label, el("span", { class: "detail" }, detail)));
}

/** Seals and sends the share; null once it is made, else the words for why not. */
async function share(context: ShareContext, to: NonNullable<ShareTarget["recipient"]>, right: "read" | "edit"): Promise<string | null> {
  const { held, vouched, opener, opened } = context;
  let body: Readonly<Record<string, unknown>>;
  try {
    body = await shareBody(held.company, vouched, await opener.keyFor(opened), opened, to, right);
  } catch (problem) {
    return sealFailure(problem instanceof VaultCryptoError ? problem.reason : "malformed");
  }
  const answer = await call("POST", `${companyPath(held)}/records/${encodeURIComponent(opened.row.id)}/shares`, ignored, { body });
  return answer.ok ? null : shareFailure(answer.status, answer.code, loadFailure(answer));
}
