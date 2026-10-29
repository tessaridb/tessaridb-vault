// The phone menu: on a narrow screen the sections and the vault's controls sit behind one button in the top bar. It is
// a disclosure, not a dialog: the button says whether it is open, and going somewhere, Escape or a tap outside closes it.

export type MenuEvent = "toggle" | "navigate" | "escape" | "outside";

/** Whether the menu is open after `event`, from `open`. */
export const nextMenu = (open: boolean, event: MenuEvent): boolean => (event === "toggle" ? !open : false);

/** Wires the menu button to the panel it shows, in the page's sidebar. */
export function installMenu(window: Window): void {
  const document = window.document;
  const button = document.getElementById("menu-toggle");
  const side = document.querySelector<HTMLElement>(".side");
  if (button === null || side === null) {
    return;
  }
  let open = false;
  const apply = (event: MenuEvent): void => {
    open = nextMenu(open, event);
    side.toggleAttribute("data-open", open);
    button.setAttribute("aria-expanded", String(open));
  };
  button.addEventListener("click", () => apply("toggle"));
  window.addEventListener("hashchange", () => apply("navigate"));
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && open) {
      apply("escape");
      button.focus();
    }
  });
  document.addEventListener("click", (event) => {
    if (open && event.target instanceof Node && !side.contains(event.target)) {
      apply("outside");
    }
  });
}
