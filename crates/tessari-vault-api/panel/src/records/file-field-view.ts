// A file field: in the edit form, the file it holds and a way to choose one, sealed and sent as soon as it is chosen; on
// the record page, Download opens it here and hands it to the browser to save. A file belongs to a saved record, so a
// record not saved yet says to save it first.

import { announce, el, fill } from "../dom.ts";
import type { Unlocked } from "../vault.ts";
import type { Control } from "./custom-fields-view.ts";
import type { FileValue } from "./fields.ts";
import { downloadFile } from "./file-io.ts";

/** Sends a chosen file and answers the field's new value, or why it wasn't kept. */
export type Attach = (file: File) => Promise<{ readonly ok: true; readonly value: FileValue } | { readonly ok: false; readonly words: string }>;

const sizeWords = (bytes: number): string => new Intl.NumberFormat(undefined, { style: "unit", unit: "kilobyte" }).format(Math.ceil(bytes / 1024));

/** A file's name and size, as the form and the record page show it. */
export const fileWords = (value: FileValue): string => `${value.name} (${sizeWords(value.size)})`;

/** The control for the file field `label`, holding `initial`; `attach` is null while the record isn't saved yet. */
export function fileControl(label: string, initial: FileValue | null, id: string, attach: Attach | null): Control {
  let value = initial;
  const name = el("label", { for: id }, label);
  const shown = el("p", {});
  const status = el("p", { class: "muted", role: "status" });
  const hint = el("p", { id: `${id}-hint`, class: "message bad" });
  hint.hidden = true;
  const mark = (words: string | null): void => {
    hint.hidden = words === null;
    hint.textContent = words ?? "";
  };
  const draw = (): void => {
    fill(shown, value === null ? el("span", { class: "muted" }, "No file yet.") : fileWords(value));
  };
  draw();
  if (attach === null) {
    const later = el("p", { class: "muted", id }, "Save the record first, then add its file.");
    return { row: el("div", { class: "field" }, el("p", { class: "field-label" }, label), shown, later), read: () => value, mark };
  }
  const input = el("input", { id, type: "file", "aria-describedby": hint.id });
  input.addEventListener("change", () => {
    const chosen = input.files?.[0];
    if (chosen === undefined) {
      return;
    }
    mark(null);
    input.disabled = true;
    status.textContent = `Sealing and sending ${chosen.name}.`;
    void attach(chosen).then((made) => {
      input.disabled = false;
      input.value = "";
      if (!made.ok) {
        status.textContent = "";
        mark(made.words);
        return;
      }
      const had = value !== null;
      value = made.value;
      draw();
      status.textContent = had ? `${chosen.name} is sent. It replaces the old file when you save the record.` : `${chosen.name} is sent. Save the record to keep it.`;
    });
  });
  return { row: el("div", { class: "field" }, name, shown, input, status, hint), read: () => value, mark };
}

/** Opens the file `value` names on record `record` and gives it to the browser to save. */
export async function saveToDevice(held: Unlocked, record: string, value: FileValue): Promise<void> {
  announce(`Opening ${value.name}.`);
  const opened = await downloadFile(held, record, value);
  if (!opened.ok) {
    announce(opened.words);
    return;
  }
  const url = URL.createObjectURL(new Blob([opened.value], { type: value.mediaType }));
  const link = el("a", { href: url, download: value.name });
  document.body.append(link);
  link.click();
  link.remove();
  // The browser has taken the file; the page's copy goes with the URL.
  setTimeout(() => URL.revokeObjectURL(url), 0);
  announce(`${value.name} is ready in your downloads.`);
}
