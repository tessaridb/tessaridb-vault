// The Company page: where an owner deletes the company, or cancels a deletion that waits. A deletion is signed in this
// browser by the owner's own key, needs the company's exact name and a reason, and is carried out only after the
// platform admin confirms it and seven days pass, during which the company is read-only and any owner may cancel.

import { call, ignored } from "../api.ts";
import { el, field, fill } from "../dom.ts";
import { loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { deletionReady, dropFailure, readStanding, signedDrop, type Standing } from "./company-model.ts";

const WHAT_HAPPENS =
  "The platform admin confirms the deletion. From then on the company is read-only for seven days, and any owner can cancel. After that its records, files, members and groups are deleted for good; its trail is kept. Backups taken before the deletion still hold the records, sealed, until the service key is next rotated.";

/** Draws the Company page for `held` into `main`; `notice` is said once at the top. */
export async function drawCompany(main: HTMLElement, held: Unlocked, notice: string | null = null): Promise<void> {
  const head = el("div", { class: "page-head" }, el("div", {}, heading("Company"), el("p", { class: "lede" }, held.companyName)));
  fill(main, head, el("p", { class: "muted", role: "status" }, "Loading the company."));
  const base = `${companyPath(held)}/drop`;
  const answer = await call("GET", base, readStanding);
  const said = el("div", { "aria-live": "polite" }, notice === null ? null : el("p", { class: "muted", role: "status" }, notice));
  const again = (words: string | null): void => void drawCompany(main, held, words);
  if (answer.ok) {
    fill(main, head, said, waitingSection(held, base, answer.value, said, again));
    return;
  }
  if (answer.code === "not_found") {
    fill(main, head, said, deleteSection(held, base, said, again));
    return;
  }
  fill(main, head, said, refusal(answer.code === "forbidden" ? dropFailure(answer) : loadFailure(answer)));
}

function waitingSection(held: Unlocked, base: string, standing: Standing, said: HTMLElement, again: (words: string | null) => void): HTMLElement {
  const lead =
    standing.state === "requested"
      ? "An owner asked for this company to be deleted. It waits for the platform admin to confirm."
      : `This company is read-only and will be deleted after ${new Date(standing.dropAfter * 1000).toLocaleString()}.`;
  const cancel = el("button", { type: "button" }, "Cancel the deletion");
  cancel.addEventListener("click", () => {
    cancel.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Signing and sending."));
    void signedDrop(held.keys.signingPrivate, "cancel", held.company)
      .then((body) => call("POST", `${base}/cancel`, ignored, { body }))
      .then((sent) => {
        cancel.disabled = false;
        return sent.ok ? again("The deletion is cancelled. The company is open again.") : fill(said, refusal(dropFailure(sent)));
      });
  });
  return el("section", { class: "record-section", "aria-labelledby": "company-deletion" }, el("h2", { id: "company-deletion" }, "Deletion"), el("p", {}, lead), cancel);
}

function deleteSection(held: Unlocked, base: string, said: HTMLElement, again: (words: string | null) => void): HTMLElement {
  const named = field("company-name", "Type the company's name to confirm", { autocomplete: "off" }, held.companyName);
  const why = field("company-reason", "Why it is being deleted", { autocomplete: "off" }, "Kept on the company's trail.");
  const confirmed = el("button", { type: "button", class: "danger", disabled: "" }, `Delete ${held.companyName}`);
  const keep = el("button", { type: "button", autofocus: "" }, "Keep the company");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": "delete-company-title" },
    el("h2", { id: "delete-company-title" }, `Delete ${held.companyName}?`),
    el("p", {}, WHAT_HAPPENS),
    named.row,
    why.row,
    el("div", { class: "actions" }, keep, confirmed),
  );
  const ready = (): void => {
    confirmed.disabled = !deletionReady(named.input.value, why.input.value, held.companyName);
  };
  named.input.addEventListener("input", ready);
  why.input.addEventListener("input", ready);
  keep.addEventListener("click", () => dialog.close());
  confirmed.addEventListener("click", () => {
    if (!deletionReady(named.input.value, why.input.value, held.companyName)) {
      return;
    }
    const reason = why.input.value.trim();
    dialog.close();
    fill(said, el("p", { class: "muted", role: "status" }, "Signing and sending."));
    void signedDrop(held.keys.signingPrivate, "request", held.company)
      .then((body) => call("POST", base, ignored, { body, reason }))
      .then((sent) => (sent.ok ? again("The deletion is asked for. It waits for the platform admin to confirm.") : fill(said, refusal(dropFailure(sent)))));
  });
  const open = el("button", { type: "button", class: "danger" }, "Delete this company");
  open.addEventListener("click", () => {
    named.input.value = "";
    why.input.value = "";
    ready();
    dialog.showModal();
  });
  return el("section", { class: "record-section", "aria-labelledby": "company-delete" }, el("h2", { id: "company-delete" }, "Delete the company"), el("p", { class: "muted" }, WHAT_HAPPENS), open, dialog);
}
