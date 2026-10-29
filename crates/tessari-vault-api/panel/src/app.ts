// The web application's entry point: one view at a time, chosen by the router from the URL, so every view is a link a
// member can reload or hand to a colleague. Views are drawn with text nodes only; nothing a server sends is ever
// parsed as markup.

import { call, ignored } from "./api.ts";
import { announce } from "./dom.ts";
import { afterSignIn, afterUnlock, every, needsSession, needsUnlock } from "./guards.ts";
import { browserHistory } from "./router/history.ts";
import { Router, defineRoutes, type Outcome } from "./router/router.ts";
import type { Stops } from "./records/field-view.ts";
import { drawEdit, drawNew } from "./records/edit-view.ts";
import { drawList } from "./records/list-view.ts";
import { copier } from "./records/listing.ts";
import { drawRecord } from "./records/record-view.ts";
import { text } from "./router/query.ts";
import { drawMembers } from "./members/members-view.ts";
import { bearer, forget } from "./session.ts";
import { providerReturned, signIn } from "./sign-in.ts";
import { unlock } from "./unlock.ts";
import { IDLE_MINUTES, lock, lockWhenIdle, onLock, unlocked } from "./vault.ts";
import { installTips } from "./icon-action.ts";
import { decorateShell, setIcon } from "./shell-icons.ts";

const history = browserHistory(window);
/** What the identity provider's return refused, said once on the sign-in page. */
let notice: string | null = null;
const signedIn = (): boolean => bearer() !== null;
const isUnlocked = (): boolean => unlocked() !== null;
const session = needsSession(signedIn, () => history.current());
const open = every(session, needsUnlock(isUnlocked, () => history.current()));

const routes = defineRoutes({
  records: { path: "/", guard: open },
  newRecord: { path: "/records/new", query: { type: text }, guard: open },
  record: { path: "/records/:record", guard: open },
  editRecord: { path: "/records/:record/edit", guard: open },
  members: { path: "/members", guard: open },
  signIn: { path: "/sign-in", query: { next: text } },
  unlock: { path: "/unlock", query: { next: text }, guard: session },
});

type Routes = typeof routes;

function heading(view: HTMLElement, title: string, lead: string): void {
  const h1 = document.createElement("h1");
  h1.textContent = title;
  const p = document.createElement("p");
  p.textContent = lead;
  view.replaceChildren(h1, p);
}

/** Copied values leave the clipboard by themselves, and at once when the vault locks. */
const copy = copier({ write: (value) => navigator.clipboard.writeText(value) }, { set: (run, ms) => window.setTimeout(run, ms), clear: (handle) => window.clearTimeout(Number(handle)) }, announce);
/** The timers of the view on screen, stopped before the next view is drawn. */
const stops: Stops = [];
const stopView = (): void => {
  for (const stop of stops.splice(0)) {
    stop();
  }
};

/** Opens `href`, a hash of this page; the router hears of it as it hears of a link. */
const go = (href: string): void => {
  window.location.hash = href;
};

function draw(outcome: Outcome<Routes>, view: HTMLElement, router: Router<Routes>): void {
  stopView();
  // Before signing in and while locked there is nothing to navigate: the card is the page.
  document.body.classList.toggle("signed-out", !signedIn() || !isUnlocked());
  showCompany();
  if (outcome.kind !== "matched") {
    heading(view, "Not found", "This address names no page of the vault.");
    return;
  }
  switch (outcome.match.name) {
    case "records": {
      const held = unlocked();
      if (held !== null) {
        void drawList(view, held, (record) => router.href("record", { record }), router.href("newRecord", {}), stops);
      }
      break;
    }
    case "record": {
      const held = unlocked();
      if (held !== null) {
        const record = outcome.match.params.record;
        void drawRecord(view, held, record, copy, stops, router.href("records", {}), router.href("editRecord", { record }), () => go(router.href("records", {})));
      }
      break;
    }
    case "newRecord": {
      const held = unlocked();
      if (held !== null) {
        const opened = (record: string): void => go(router.href("record", { record }));
        drawNew(view, held, outcome.match.query.type, router.href("records", {}), opened, (type) => router.href("newRecord", {}, { type }));
      }
      break;
    }
    case "editRecord": {
      const held = unlocked();
      if (held !== null) {
        const record = outcome.match.params.record;
        void drawEdit(view, held, record, router.href("record", { record }), (saved) => go(router.href("record", { record: saved })));
      }
      break;
    }
    case "members": {
      const held = unlocked();
      if (held !== null) {
        void drawMembers(view, held);
      }
      break;
    }
    case "signIn": {
      if (signedIn()) {
        go(afterSignIn(outcome.match.query.next));
        return;
      }
      const next = outcome.match.query.next;
      signIn(view, () => go(afterSignIn(next)), notice);
      notice = null;
      break;
    }
    case "unlock": {
      const next = outcome.match.query.next;
      if (isUnlocked()) {
        go(afterUnlock(next));
        return;
      }
      void unlock(view, () => go(afterUnlock(next)), () => void signOut());
      break;
    }
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

/** The unlocked company's name in the sidebar, and "Switch company" when the person is in others too. */
function showCompany(): void {
  const held = unlocked();
  const name = document.getElementById("company");
  const workspace = document.getElementById("workspace");
  const switcher = document.getElementById("switch-company");
  if (name !== null && workspace !== null) {
    name.textContent = held?.companyName ?? "";
    workspace.hidden = held === null;
  }
  if (switcher !== null) {
    switcher.hidden = held?.otherCompanies !== true;
  }
}

/** Switching company locks this one's keys and asks which company to unlock, starting from its records. */
function switchButton(): void {
  document.getElementById("switch-company")?.addEventListener("click", () => {
    lock();
    go(`#/unlock?next=${encodeURIComponent("/")}`);
  });
}

/** The person's choice of theme, kept on <html> and in this browser only. */
function themed(): void {
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

/** Signing out locks the vault and ends the session on the server, then here, whatever the server answered. */
async function signOut(): Promise<void> {
  lock();
  await call("DELETE", "/api/session", ignored);
  forget();
  go("#/sign-in");
}

function signOutButton(): void {
  document.getElementById("sign-out")?.addEventListener("click", () => void signOut());
}

/** Locking, by the button or after a stretch with no input, takes the person to unlock and back to where they were. */
function locking(): void {
  document.getElementById("lock")?.addEventListener("click", lock);
  lockWhenIdle(window, IDLE_MINUTES, { set: (run, ms) => window.setTimeout(run, ms), clear: (handle) => window.clearTimeout(Number(handle)) });
  onLock(() => {
    stopView();
    void copy.clearNow();
    if (signedIn()) {
      go(`#/unlock?next=${encodeURIComponent(history.current().replace(/^#/, ""))}`);
    }
  });
}

const view = document.getElementById("view");
if (view !== null) {
  decorateShell();
  installTips();
  themed();
  signOutButton();
  switchButton();
  locking();
  // An identity provider's return is finished before any route is drawn, so the sign-in page never flashes first.
  void providerReturned().then((refused) => {
    notice = refused;
    const router = new Router(routes, history);
    router.start((outcome) => {
      draw(outcome, view, router);
      mark(outcome);
    });
  });
  // The skip link must move focus, not the hash: the hash is the route.
  document.getElementById("skip")?.addEventListener("click", (event) => {
    event.preventDefault();
    view.focus();
  });
}
