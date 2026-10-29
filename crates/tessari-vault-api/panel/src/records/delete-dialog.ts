// Deleting a record from its page: a native dialog that names the record, says what stays behind (backups taken
// earlier keep a sealed copy until they expire) and takes an optional reason for the company's trail. The page asks;
// the server decides who may.

import { call, ignored } from "../api.ts";
import { el, fill } from "../dom.ts";
import { iconAction } from "../icon-action.ts";
import { refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { deleteFailure } from "./delete-words.ts";
import { loadFailure } from "./list-view.ts";
import { companyPath } from "./load.ts";

/** The "Delete" button for record `id` titled `title`, and the dialog it opens; `gone` runs once it is deleted. */
export function deleteControls(held: Unlocked, id: string, title: string, gone: () => void): readonly HTMLElement[] {
  const reason = el("input", { id: "delete-reason", type: "text", autocomplete: "off", maxlength: "500" });
  const said = el("div", { "aria-live": "polite" });
  const keep = el("button", { type: "button", autofocus: "" }, "Keep the record");
  const confirm = el("button", { type: "button", class: "danger" }, "Delete record");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": "delete-title" },
    el("h2", { id: "delete-title" }, `Delete \u201C${title}\u201D?`),
    el(
      "p",
      {},
      "The record, its files and its history are removed for everyone it is shared with. Backups taken before now keep a sealed copy until they expire.",
    ),
    el("div", { class: "field" }, el("label", { for: reason.id }, "Reason, kept on the company's trail (optional)"), reason),
    said,
    el("div", { class: "actions" }, keep, confirm),
  );
  const open = iconAction("trash", "Delete", `Delete ${title}`, "danger");
  open.addEventListener("click", () => {
    fill(said, null);
    reason.value = "";
    dialog.showModal();
  });
  keep.addEventListener("click", () => dialog.close());
  confirm.addEventListener("click", () => {
    confirm.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Deleting the record."));
    void call("DELETE", `${companyPath(held)}/records/${encodeURIComponent(id)}`, ignored, { reason: reason.value.trim() }).then((answer) => {
      confirm.disabled = false;
      if (answer.ok) {
        dialog.close();
        gone();
        return;
      }
      fill(said, refusal(deleteFailure(answer.status, answer.code, loadFailure(answer))));
    });
  });
  return [open, dialog];
}
