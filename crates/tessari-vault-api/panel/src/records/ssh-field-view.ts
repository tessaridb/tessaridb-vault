// An SSH key field in the edit form: the key it holds, shown by its public line and fingerprint, and two ways to set
// it: make a new Ed25519 key here, or paste an OpenSSH private key file. Making or reading a key takes a moment, so the
// key is settled when the person acts and read back when the form saves. A pasted file leaves the page once read.

import { el, fill } from "../dom.ts";
import type { Control } from "./custom-fields-view.ts";
import type { SshKeyValue } from "./fields.ts";
import { sshImportProblem } from "./ssh-field.ts";
import { generateSshKey, importSshKey } from "./ssh.ts";

/** The control for the SSH key field `label`, holding `initial`; a new key is commented with the field's label. */
export function sshKeyControl(label: string, initial: SshKeyValue | null, id: string): Control {
  let value = initial;
  const name = el("p", { id: `${id}-label`, class: "field-label" }, label);
  const shown = el("div", { class: "ssh-key" });
  const status = el("p", { class: "muted", role: "status" });
  const hint = el("p", { id: `${id}-hint`, class: "message bad" });
  hint.hidden = true;
  const make = el("button", { type: "button" });
  const paste = el("button", { type: "button", "aria-expanded": "false", "aria-controls": `${id}-paste` });
  const area = el("textarea", { id, rows: "6", spellcheck: "false", autocomplete: "off", "aria-describedby": hint.id });
  const use = el("button", { type: "button", class: "primary" }, "Use this key");
  const pasting = el("div", { id: `${id}-paste`, class: "field" }, el("label", { for: id }, "Private key file"), area, el("div", { class: "input-line" }, use));
  pasting.hidden = true;
  const mark = (words: string | null): void => {
    hint.hidden = words === null;
    hint.textContent = words ?? "";
    if (words === null) {
      area.removeAttribute("aria-invalid");
    } else {
      area.setAttribute("aria-invalid", "true");
    }
  };
  const draw = (): void => {
    fill(shown, ...(value === null ? [el("p", { class: "muted" }, "No key yet.")] : [el("p", { class: "value mono" }, value.publicKey), el("p", { class: "muted mono" }, value.fingerprint)]));
    make.textContent = value === null ? "Make a key" : "Make a new key";
    paste.textContent = value === null ? "Paste a key" : "Paste another key";
  };
  const replaced = (next: SshKeyValue, words: string): void => {
    const had = value !== null;
    value = next;
    draw();
    status.textContent = had ? `${words} It replaces the old key when you save the record.` : words;
  };
  make.addEventListener("click", () => {
    make.disabled = true;
    status.textContent = "Making a key.";
    void generateSshKey(label).then(
      (key) => {
        make.disabled = false;
        replaced(key, "A new key was made.");
      },
      () => {
        make.disabled = false;
        status.textContent = "";
        mark("This browser couldn't make an Ed25519 key. Paste one made with ssh-keygen -t ed25519 instead.");
      },
    );
  });
  paste.addEventListener("click", () => {
    const open = pasting.hidden;
    pasting.hidden = !open;
    paste.setAttribute("aria-expanded", String(open));
    if (open) {
      area.focus();
    }
  });
  use.addEventListener("click", () => {
    mark(null);
    void importSshKey(area.value).then(
      (key) => {
        area.value = "";
        pasting.hidden = true;
        paste.setAttribute("aria-expanded", "false");
        replaced(key, "The pasted key is in place.");
        paste.focus();
      },
      (problem: unknown) => mark(sshImportProblem(problem)),
    );
  });
  draw();
  const row = el("div", { class: "field", role: "group", "aria-labelledby": name.id }, name, shown, el("div", { class: "input-line" }, make, paste), pasting, status, hint);
  return { row, read: () => value, mark };
}
