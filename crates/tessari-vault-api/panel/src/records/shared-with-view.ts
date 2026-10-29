// The "Shared with" section of a record's page, shown to its editors: each member or group the record is shared with,
// what they can do, and a way to stop sharing with them. A removal is recorded with the reason given; the record's next
// revision is sealed under a new key, which every revision this page makes is already.

import { call, ignored } from "../api.ts";
import { el, fill } from "../dom.ts";
import { iconAction } from "../icon-action.ts";
import { refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { loadFailure } from "./list-view.ts";
import { companyPath } from "./load.ts";
import type { SharedRow } from "./share-targets.ts";
import { removeShareFailure } from "./share-words.ts";

/** The section listing `rows` for record `id`, and the one dialog its stop-sharing buttons open; `done` runs after a removal. */
export function sharedWithSection(held: Unlocked, id: string, rows: readonly SharedRow[], done: () => void): readonly HTMLElement[] {
  const reason = el("input", { id: "unshare-reason", type: "text", autocomplete: "off", maxlength: "500" });
  const said = el("div", { "aria-live": "polite" });
  const title = el("h2", { id: "unshare-title" });
  const keep = el("button", { type: "button", autofocus: "" }, "Keep sharing");
  const confirm = el("button", { type: "button", class: "danger" }, "Stop sharing");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": title.id },
    title,
    el(
      "p",
      {},
      "They can no longer open this record. What they have already seen, and backups taken before now, are not taken back. The record's next change is sealed under a new key.",
    ),
    el("div", { class: "field" }, el("label", { for: reason.id }, "Reason, kept on the company's trail (optional)"), reason),
    said,
    el("div", { class: "actions" }, keep, confirm),
  );
  let removing: SharedRow | null = null;
  keep.addEventListener("click", () => dialog.close());
  confirm.addEventListener("click", () => {
    if (removing === null) {
      return;
    }
    confirm.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Removing the share."));
    const route = `${companyPath(held)}/records/${encodeURIComponent(id)}/shares/${encodeURIComponent(removing.target)}`;
    void call("DELETE", route, ignored, { reason: reason.value.trim() }).then((answer) => {
      confirm.disabled = false;
      if (answer.ok) {
        dialog.close();
        done();
        return;
      }
      fill(said, refusal(removeShareFailure(answer.status, answer.code, loadFailure(answer))));
    });
  });
  const items = rows.map((row) => {
    const remove = iconAction("close", "Stop sharing", `Stop sharing with ${row.label}`, "danger");
    remove.addEventListener("click", () => {
      removing = row;
      fill(title, `Stop sharing with \u201C${row.label}\u201D?`);
      fill(said, null);
      reason.value = "";
      dialog.showModal();
    });
    return el(
      "li",
      { class: "share" },
      el("div", {}, el("span", { class: "share-name" }, row.label), row.detail === "" ? null : el("span", { class: "detail" }, row.detail)),
      el("span", { class: "muted" }, row.right === "edit" ? "Can edit" : "Can view"),
      remove,
    );
  });
  const section = el(
    "section",
    { class: "record-section", "aria-labelledby": "shared-with" },
    el("h2", { id: "shared-with" }, "Shared with"),
    items.length === 0 ? el("p", { class: "muted" }, "It isn't shared with anyone; only its author can open it.") : el("ul", { class: "shares" }, ...items),
  );
  return [section, dialog];
}
