// Making or changing a record: a form drawn from a type's template or from the opened record, a control per field
// type, every value checked before it is sealed, and the save sealed here and sent as ciphertext. A value refused is
// said beside its field; the form keeps everything typed.

import { call } from "../api.ts";
import { el, fill } from "../dom.ts";
import { readOpened, readRow, type RecordRow } from "../records-model.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import type { RecordDocument } from "./document.ts";
import { edited, fieldAt } from "./editing.ts";
import type { Field } from "./fields.ts";
import { CHAIN_REFUSED, loadFailure } from "./list-view.ts";
import { typeName, unreadableWords } from "./listing.ts";
import { Opener, companyPath, vouchedChain } from "./load.ts";
import { newRecord, revisedRecord } from "./save-record.ts";
import { BUILT_IN_TYPES, recordFromTemplate } from "./types.ts";

const INPUT: Partial<Record<Field["type"], string>> = {
  text: "text",
  concealed: "password",
  url: "url",
  email: "email",
  phone: "tel",
  number: "text",
  date: "date",
  "month-year": "month",
};

/** One field's control and a way to read what it holds; types the form cannot edit yet keep their value. */
function control(field: Field, id: string): { readonly row: HTMLElement; readonly read: () => unknown; readonly mark: (words: string | null) => void } {
  const hint = el("p", { id: `${id}-hint`, class: "message bad" });
  hint.hidden = true;
  const label = el("label", { for: id }, field.label);
  let input: HTMLInputElement | HTMLTextAreaElement | null = null;
  let read: () => unknown = () => field.value;
  if (field.type === "boolean") {
    const box = el("input", { id, type: "checkbox" });
    box.checked = field.value === true;
    read = () => box.checked;
    input = box;
  } else if (field.type === "multiline") {
    const area = el("textarea", { id, rows: "4" });
    area.value = field.value ?? "";
    read = () => area.value;
    input = area;
  } else if (INPUT[field.type] !== undefined && (typeof field.value === "string" || field.value === null)) {
    const text = el("input", { id, type: INPUT[field.type] ?? "text", autocomplete: field.type === "concealed" ? "new-password" : "off" });
    text.value = typeof field.value === "string" ? field.value : "";
    read = () => text.value;
    input = text;
  }
  input?.setAttribute("aria-describedby", hint.id);
  const kept = input === null ? el("p", { class: "muted" }, "Kept as it is. Editing this kind of field comes later.") : null;
  const mark = (words: string | null): void => {
    hint.hidden = words === null;
    hint.textContent = words ?? "";
    if (words === null) {
      input?.removeAttribute("aria-invalid");
    } else {
      input?.setAttribute("aria-invalid", "true");
    }
  };
  return { row: el("div", { class: "field" }, label, input, kept, hint), read, mark };
}

function saveFailure(code: string, fallback: string): string {
  switch (code) {
    case "stale_revision":
      return "This record changed since you opened it. Open it again to see the newer version, then make your change.";
    case "stale_shares":
      return "This record is shared, and this page can't yet carry a change to the people it is shared with.";
    default:
      return fallback;
  }
}

/** The form for `document`; `save` seals and sends it and answers a refusal in words, or null when it is kept. */
function form(main: HTMLElement, title: string, document: RecordDocument, back: { readonly href: string; readonly label: string }, save: (made: RecordDocument) => Promise<string | null>): void {
  const name = el("input", { id: "record-title", type: "text", autocomplete: "off", value: document.title });
  const nameHint = el("p", { id: "record-title-hint", class: "message bad" });
  nameHint.hidden = true;
  name.setAttribute("aria-describedby", nameHint.id);
  const controls = new Map<string, ReturnType<typeof control>>();
  const sections = document.sections.map((section, sectionIndex) =>
    el(
      "fieldset",
      { class: "record-section" },
      section.name === "" ? null : el("legend", {}, section.name),
      ...section.fields.map((field, fieldIndex) => {
        const at = fieldAt(sectionIndex, fieldIndex);
        const made = control(field, `field-${sectionIndex}-${fieldIndex}`);
        controls.set(at, made);
        return made.row;
      }),
    ),
  );
  const submit = el("button", { type: "submit", class: "primary" }, "Save record");
  const said = el("div", { "aria-live": "polite" });
  const body = el("form", { novalidate: "" }, el("div", { class: "field" }, el("label", { for: name.id }, "Title"), name, nameHint), ...sections, submit, said);
  body.addEventListener("submit", (event) => {
    event.preventDefault();
    const values = new Map([...controls].map(([at, made]) => [at, made.read()] as const));
    const result = edited(document, name.value, values);
    nameHint.hidden = result.ok || !result.problems.has("title");
    nameHint.textContent = result.ok ? "" : (result.problems.get("title") ?? "");
    for (const [at, made] of controls) {
      made.mark(result.ok ? null : (result.problems.get(at) ?? null));
    }
    if (!result.ok) {
      fill(said, refusal("Some values can't be kept. Each one says why below its label."));
      body.querySelector<HTMLElement>("[aria-invalid=true]")?.focus();
      if (result.problems.has("title")) {
        name.setAttribute("aria-invalid", "true");
        name.focus();
      }
      return;
    }
    name.removeAttribute("aria-invalid");
    submit.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Sealing and saving the record."));
    void save(result.document).then((refused) => {
      submit.disabled = false;
      fill(said, refused === null ? null : refusal(refused));
    });
  });
  const head = el("div", { class: "page-head" }, el("div", {}, heading(title), el("p", { class: "lede" }, typeName(document.type))));
  fill(main, el("a", { href: back.href, class: "back" }, back.label), head, body);
  name.focus();
}

/** A new record of built-in `type`, or the choice of type when none is named. */
export function drawNew(main: HTMLElement, held: Unlocked, type: string | null, back: string, opened: (record: string) => void, newOf: (type: string) => string): void {
  const template = BUILT_IN_TYPES.find((each) => each.id === type);
  if (template === undefined) {
    const choices = el("ul", { class: "records", "aria-label": "Record types" }, ...BUILT_IN_TYPES.map((each) => el("li", { class: "record" }, el("a", { class: "record-title", href: newOf(each.id) }, each.name))));
    fill(main, el("a", { href: back, class: "back" }, "All records"), el("div", { class: "page-head" }, heading("New record")), el("p", {}, "Choose what kind of record to make."), choices);
    return;
  }
  form(main, "New record", recordFromTemplate(template, ""), { href: back, label: "All records" }, async (made) => {
    const record = await newRecord(held.company, held.keys, made);
    const answer = await call("POST", `${companyPath(held)}/records`, readRow, { body: record.body });
    if (!answer.ok) {
      return saveFailure(answer.code, loadFailure(answer));
    }
    opened(record.id);
    return null;
  });
}

/** The opened record `id` in a form; the save is the next revision. */
export async function drawEdit(main: HTMLElement, held: Unlocked, id: string, back: string, opened: (record: string) => void): Promise<void> {
  fill(main, el("div", { class: "page-head" }, heading("Edit record")), el("p", { class: "muted", role: "status" }, "Opening the record."));
  const [chain, answer] = await Promise.all([vouchedChain(held), call("GET", `${companyPath(held)}/records/${encodeURIComponent(id)}`, readOpened)]);
  if (!chain.ok) {
    fill(main, el("div", { class: "page-head" }, heading("Edit record")), refusal("problem" in chain ? CHAIN_REFUSED : loadFailure(chain)));
    return;
  }
  if (!answer.ok) {
    fill(main, el("div", { class: "page-head" }, heading("Edit record")), refusal(loadFailure(answer)));
    return;
  }
  const item = await new Opener(held, chain.vouched).open(answer.value);
  if (!("document" in item)) {
    fill(main, el("div", { class: "page-head" }, heading("Record not shown")), refusal(unreadableWords(item.problem)));
    return;
  }
  const row: RecordRow = item.row;
  form(main, "Edit record", item.document, { href: back, label: "Back to the record" }, async (made) => {
    const revision = await revisedRecord(held.company, held.keys, chain.vouched, row, made, []);
    const saved = await call("PUT", `${companyPath(held)}/records/${encodeURIComponent(id)}`, readRow, { body: revision });
    if (!saved.ok) {
      return saveFailure(saved.code, loadFailure(saved));
    }
    opened(id);
    return null;
  });
}
