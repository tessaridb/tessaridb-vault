// One field of an opened record, drawn with text nodes only. A concealed value stays hidden until "Show", every value
// has its own Copy, and a one-time password shows its current code and the seconds it has left.

import { el } from "../dom.ts";
import type { copier } from "./listing.ts";
import type { Field } from "./fields.ts";
import { base32Decode, totpCode } from "./totp.ts";

type Copier = ReturnType<typeof copier>;

/** Timers a view starts; the view stops them when it is left or the vault locks. */
export type Stops = Array<() => void>;

const MASK = "••••••••••••";

function copyButton(copy: Copier, label: string, value: () => string): HTMLElement {
  const button = el("button", { type: "button", class: "quiet" }, "Copy");
  button.setAttribute("aria-label", `Copy ${label}`);
  button.addEventListener("click", () => void copy.copy(value(), label));
  return button;
}

/** A hidden value with Show / Hide; the value is in the page only while shown. */
function concealed(label: string, value: string, copy: Copier): HTMLElement {
  const shown = el("span", { class: "value mono" }, MASK);
  const toggle = el("button", { type: "button", class: "quiet", "aria-pressed": "false" }, "Show");
  toggle.setAttribute("aria-label", `Show ${label}`);
  toggle.addEventListener("click", () => {
    const showing = toggle.getAttribute("aria-pressed") !== "true";
    toggle.setAttribute("aria-pressed", String(showing));
    toggle.textContent = showing ? "Hide" : "Show";
    shown.textContent = showing ? value : MASK;
  });
  return el("span", { class: "value-row" }, shown, toggle, copyButton(copy, label, () => value));
}

function plain(label: string, value: string, copy: Copier, link = false): HTMLElement {
  const safeLink = link && /^https?:\/\//i.test(value);
  const shown = safeLink
    ? el("a", { class: "value", href: value, target: "_blank", rel: "noopener noreferrer" }, value)
    : el("span", { class: "value" }, value);
  return el("span", { class: "value-row" }, shown, copyButton(copy, label, () => value));
}

/** The current one-time code and its seconds left, redrawn every second until the view stops it. */
function oneTime(label: string, value: Extract<Field, { type: "otp" }>["value"] & object, copy: Copier, stops: Stops): HTMLElement {
  const code = el("span", { class: "value mono" }, "");
  const left = el("span", { class: "muted" }, "");
  let current = "";
  const params = { secret: base32Decode(value.secret), algorithm: value.algorithm, digits: value.digits, period: value.period };
  const tick = async (): Promise<void> => {
    const now = Math.floor(Date.now() / 1000);
    current = await totpCode(params, now);
    code.textContent = `${current.slice(0, current.length / 2)} ${current.slice(current.length / 2)}`;
    left.textContent = `${value.period - (now % value.period)} s left`;
  };
  void tick();
  const timer = window.setInterval(() => void tick(), 1000);
  stops.push(() => window.clearInterval(timer));
  return el("span", { class: "value-row" }, code, left, copyButton(copy, label, () => current));
}

/** The field as a definition-list pair: its label and its value with the actions it offers. */
export function fieldView(field: Field, copy: Copier, stops: Stops): HTMLElement[] {
  const term = el("dt", {}, field.label);
  if (field.value === null) {
    return [term, el("dd", { class: "muted" }, "Empty")];
  }
  switch (field.type) {
    case "concealed":
      return [term, el("dd", {}, concealed(field.label, field.value, copy))];
    case "url":
      return [term, el("dd", {}, plain(field.label, field.value, copy, true))];
    case "multiline":
      return [term, el("dd", {}, el("span", { class: "value-row" }, el("span", { class: "value pre" }, field.value), copyButton(copy, field.label, () => field.value ?? "")))];
    case "boolean":
      return [term, el("dd", {}, field.value ? "Yes" : "No")];
    case "otp":
      return [term, el("dd", {}, oneTime(field.label, field.value, copy, stops))];
    case "file":
      return [term, el("dd", {}, `${field.value.name} (${new Intl.NumberFormat(undefined, { style: "unit", unit: "kilobyte" }).format(Math.ceil(field.value.size / 1024))})`)];
    case "security-question":
      return [term, el("dd", {}, el("p", {}, field.value.question), concealed(`${field.label} answer`, field.value.answer, copy))];
    case "ssh-key":
      return [
        term,
        el(
          "dd",
          {},
          plain(`${field.label} public key`, field.value.publicKey, copy),
          el("p", { class: "muted mono" }, field.value.fingerprint),
          concealed(`${field.label} private key`, field.value.privateKey, copy),
        ),
      ];
    default:
      return [term, el("dd", {}, plain(field.label, field.value, copy))];
  }
}
