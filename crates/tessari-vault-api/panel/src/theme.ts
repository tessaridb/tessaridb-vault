// The person's choice of theme: dark or light, kept on <html> and in this browser only.

import { setIcon } from "./shell-icons.ts";

/** The person's choice of theme, kept on <html> and in this browser only. */
export function themed(): void {
  const button = document.getElementById("theme");
  const root = document.documentElement;
  const label = (): void => {
    const words = button?.querySelector("span");
    if (button !== null && words !== null && words !== undefined) {
      const dark = root.dataset.theme === "dark";
      words.textContent = dark ? "Light theme" : "Dark theme";
      setIcon(button, dark ? "sun" : "moon");
    }
  };
  const stored = localStorage.getItem("vault-theme");
  if (stored === "dark" || stored === "light") {
    root.dataset.theme = stored;
  }
  label();
  button?.addEventListener("click", () => {
    root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark";
    localStorage.setItem("vault-theme", root.dataset.theme);
    label();
  });
}
