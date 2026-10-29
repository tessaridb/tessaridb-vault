// The sidebar's links and buttons each carry an icon beside their label; the shell names it in `data-icon`.

import { icon, type IconName } from "./icons.ts";

function isIconName(name: string | undefined): name is IconName {
  return name === "records" || name === "members" || name === "sun" || name === "moon" || name === "lock" || name === "sign-out" || name === "switch";
}

/** Puts `name` in front of `control`'s label, replacing the icon it had. */
export function setIcon(control: Element, name: IconName): void {
  control.querySelector(":scope > svg.icon")?.remove();
  control.prepend(icon(name));
}

/** Draws the icon every `[data-icon]` control in the shell names; called once. */
export function decorateShell(): void {
  for (const control of document.querySelectorAll<HTMLElement>("[data-icon]")) {
    const name = control.dataset["icon"];
    if (isIconName(name)) {
      setIcon(control, name);
    }
  }
}
