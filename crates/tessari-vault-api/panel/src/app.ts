// The web application's entry point: one view at a time, chosen by the router from the URL, so every view is a link a
// member can reload or hand to a colleague. Views are drawn with text nodes only; nothing a server sends is ever
// parsed as markup.

import { browserHistory } from "./router/history.ts";
import { Router, defineRoutes, type Outcome } from "./router/router.ts";
import { text } from "./router/query.ts";

const routes = defineRoutes({
  records: { path: "/" },
  signIn: { path: "/sign-in", query: { next: text } },
});

type Routes = typeof routes;

function heading(view: HTMLElement, title: string, lead: string): void {
  const h1 = document.createElement("h1");
  h1.textContent = title;
  const p = document.createElement("p");
  p.textContent = lead;
  view.replaceChildren(h1, p);
}

function draw(outcome: Outcome<Routes>, view: HTMLElement): void {
  if (outcome.kind !== "matched") {
    heading(view, "Not found", "This address names no page of the vault.");
    return;
  }
  switch (outcome.match.name) {
    case "records":
      heading(view, "Records", "Sign in to open your company's records.");
      break;
    case "signIn":
      heading(view, "Sign in", "Signing in arrives with the next version of this page.");
      break;
  }
}

function mark(outcome: Outcome<Routes>): void {
  const section = outcome.kind === "matched" ? outcome.match.name : "";
  for (const link of document.querySelectorAll<HTMLAnchorElement>("nav a[data-section]")) {
    if (link.dataset.section === section) {
      link.setAttribute("aria-current", "page");
    } else {
      link.removeAttribute("aria-current");
    }
  }
}

/** The person's choice of theme, kept on <html> and in this browser only. */
function themed(): void {
  const button = document.getElementById("theme");
  const root = document.documentElement;
  const label = (): void => {
    if (button !== null) {
      button.textContent = root.dataset.theme === "dark" ? "Light theme" : "Dark theme";
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

const view = document.getElementById("view");
if (view !== null) {
  themed();
  new Router(routes, browserHistory(window)).start((outcome) => {
    draw(outcome, view);
    mark(outcome);
  });
  // The skip link must move focus, not the hash: the hash is the route.
  document.getElementById("skip")?.addEventListener("click", (event) => {
    event.preventDefault();
    view.focus();
  });
}
