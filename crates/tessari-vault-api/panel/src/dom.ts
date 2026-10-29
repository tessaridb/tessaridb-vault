// Building the page. Everything the server hands over (names, titles, reasons, and every record once it is opened) is
// placed with `textContent` or as an attribute value; nothing here parses a string as markup.

type Child = Node | string | null | false;

/** A `tag` element with `attributes` and `children`; strings become text nodes. */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Readonly<Record<string, string>> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const made = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    made.setAttribute(name, value);
  }
  for (const child of children) {
    if (child !== null && child !== false) {
      made.append(child);
    }
  }
  return made;
}

/** The element with this id; throws when the shell carries none, which is a build defect rather than a state. */
export function at(id: string): HTMLElement {
  const found = document.getElementById(id);
  if (found === null) {
    throw new Error(`the page has no element #${id}`);
  }
  return found;
}

/** Replaces everything in `parent` with `children`. */
export function fill(parent: Element, ...children: Child[]): void {
  parent.replaceChildren(...children.filter((child): child is Node | string => child !== null && child !== false));
}

/** Says `words` to a screen reader through the shell's polite live region, and shows them there. */
export function announce(words: string): void {
  at("status-line").textContent = words;
}

/** A labelled text control: the label stays visible; `hint` explains the format. */
export function field(
  id: string,
  label: string,
  attributes: Readonly<Record<string, string>> = {},
  hint?: string,
): { readonly row: HTMLElement; readonly input: HTMLInputElement } {
  const input = el("input", { id, name: id, type: "text", ...attributes });
  const described = hint === undefined ? null : el("p", { id: `${id}-hint`, class: "hint" }, hint);
  if (described !== null) {
    input.setAttribute("aria-describedby", described.id);
  }
  return { row: el("div", { class: "field" }, el("label", { for: id }, label), input, described), input };
}
