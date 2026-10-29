// The edit form's own-fields section: the fields the author added, each removable, and an "Add a field" row that opens
// in place for a name and a kind (text, or a secret shown as dots) rather than in a dialog, since it is short and its
// result lands right above it.

import { el, fill } from "../dom.ts";
import { iconAction } from "../icon-action.ts";
import { icon } from "../icons.ts";
import { CUSTOM_SECTION, customLabelProblem, type CustomField } from "./custom-fields.ts";

/** What a field's control offers the form: its row, a way to read it, and a way to say why its value was refused. */
export interface Control {
  readonly row: HTMLElement;
  readonly read: () => unknown;
  readonly mark: (words: string | null) => void;
}

export interface CustomEditor {
  readonly section: HTMLElement;
  /** The fields in order, each with its control. */
  readonly current: () => ReadonlyArray<{ readonly field: CustomField; readonly control: Control }>;
}

const KINDS = [
  { type: "text", words: "Text" },
  { type: "concealed", words: "Secret" },
] as const;

/** The section for `initial`, drawing each field with `controlFor`; `taken` names the type's own fields. */
export function customEditor(initial: readonly CustomField[], controlFor: (field: CustomField, id: string) => Control, taken: readonly string[]): CustomEditor {
  let made = 0;
  const entries: Array<{ field: CustomField; control: Control; row: HTMLElement }> = [];
  const list = el("div", { class: "custom-fields" });
  const append = (field: CustomField): HTMLElement => {
    made += 1;
    const control = controlFor(field, `custom-${made}`);
    const remove = iconAction("trash", "Remove field", `Remove the field ${field.label}`, "danger");
    // The remove button sits on the input's own line, so it is plainly that field's.
    const input = control.row.querySelector("input");
    const line = el("div", { class: "input-line" });
    input?.replaceWith(line);
    line.append(...(input === null ? [] : [input]), remove);
    const row = el("div", { class: "custom-row" }, control.row);
    const entry = { field, control, row };
    remove.addEventListener("click", () => {
      entries.splice(entries.indexOf(entry), 1);
      row.remove();
      opener.focus();
    });
    entries.push(entry);
    list.append(row);
    return row;
  };
  initial.forEach(append);

  const name = el("input", { id: "custom-name", type: "text", autocomplete: "off", maxlength: "64" });
  const hint = el("p", { id: "custom-name-hint", class: "message bad" });
  hint.hidden = true;
  name.setAttribute("aria-describedby", hint.id);
  let kind: CustomField["type"] = "text";
  const chips = KINDS.map(({ type, words }) => {
    const chip = el("button", { type: "button", class: "chip", "aria-pressed": String(type === kind) }, words);
    chip.addEventListener("click", () => {
      kind = type;
      chips.forEach((other, at) => other.setAttribute("aria-pressed", String(KINDS[at]?.type === kind)));
    });
    return chip;
  });
  const add = el("button", { type: "button", class: "primary" }, "Add field");
  const cancel = el("button", { type: "button", class: "quiet" }, "Cancel");
  const panel = el(
    "div",
    { class: "custom-add" },
    el("div", { class: "field" }, el("label", { for: name.id }, "Field name"), name, hint),
    el("div", { class: "chips", role: "group", "aria-label": "Kind of field" }, ...chips),
    el("div", { class: "actions" }, add, cancel),
  );
  panel.hidden = true;
  const opener = el("button", { type: "button", class: "quiet add-field", "aria-expanded": "false" }, icon("plus"), el("span", {}, "Add a field"));
  const close = (): void => {
    panel.hidden = true;
    opener.hidden = false;
    opener.setAttribute("aria-expanded", "false");
  };
  opener.addEventListener("click", () => {
    name.value = "";
    hint.hidden = true;
    name.removeAttribute("aria-invalid");
    panel.hidden = false;
    opener.hidden = true;
    opener.setAttribute("aria-expanded", "true");
    name.focus();
  });
  const submit = (): void => {
    const problem = customLabelProblem(name.value, [...taken, ...entries.map((entry) => entry.field.label)]);
    hint.hidden = problem === null;
    fill(hint, problem ?? "");
    if (problem !== null) {
      name.setAttribute("aria-invalid", "true");
      name.focus();
      return;
    }
    const row = append({ label: name.value.trim(), type: kind, value: null });
    close();
    row.querySelector<HTMLElement>("input")?.focus();
  };
  add.addEventListener("click", submit);
  cancel.addEventListener("click", () => {
    close();
    opener.focus();
  });
  name.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
      opener.focus();
    }
  });
  const section = el("fieldset", { class: "record-section" }, el("legend", {}, CUSTOM_SECTION), list, opener, panel);
  return { section, current: () => entries.map(({ field, control }) => ({ field, control })) };
}
