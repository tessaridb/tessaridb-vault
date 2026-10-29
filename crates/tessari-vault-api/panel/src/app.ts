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
import { drawMember } from "./members/member-page.ts";
import { drawMembers } from "./members/members-view.ts";
import { drawGroup } from "./groups/group-page.ts";
import { drawGroups } from "./groups/groups-view.ts";
import { drawTrail } from "./trail/trail-view.ts";
import { drawAccount } from "./account/account-view.ts";
import { drawCompany } from "./company/company-page.ts";
import { checkNotices, dropBanner, placeBanner } from "./recovery/notice-banner.ts";
import { drawRecovery } from "./recovery/recovery-view.ts";
import { bearer, forget } from "./session.ts";
import { join } from "./join.ts";
import { providerReturned, signIn } from "./sign-in.ts";
import { unlock } from "./unlock.ts";
import { IDLE_MINUTES, lock, lockWhenIdle, onLock, unlocked, type Unlocked } from "./vault.ts";
import { installTips } from "./icon-action.ts";
import { decorateShell } from "./shell-icons.ts";
import { themed } from "./theme.ts";
import { installMenu } from "./menu.ts";
import { freshPane } from "./pane.ts";
import { sectionOf } from "./nav-section.ts";

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
  member: { path: "/members/:account", guard: open },
  groups: { path: "/groups", guard: open },
  group: { path: "/groups/:group", guard: open },
  trail: { path: "/trail", guard: open },
  recovery: { path: "/recovery", guard: open },
  account: { path: "/account", guard: open },
  company: { path: "/company", guard: open },
  signIn: { path: "/sign-in", query: { next: text } },
  join: { path: "/join", query: { token: text } },
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

/** Draws a view that needs the unlocked company, when it is unlocked; the route's guard sends a locked vault away. */
const withHeld = (drawn: (held: Unlocked) => Promise<void>): void => {
  const held = unlocked();
  if (held !== null) {
    void drawn(held);
  }
};

/** Opens `href`, a hash of this page; the router hears of it as it hears of a link. */
const go = (href: string): void => {
  window.location.hash = href;
};

function draw(outcome: Outcome<Routes>, main: HTMLElement, router: Router<Routes>): void {
  stopView();
  const view = freshPane(main, () => document.createElement("div"));
  // Before signing in and while locked there is nothing to navigate: the card is the page.
  document.body.classList.toggle("signed-out", !signedIn() || !isUnlocked());
  showCompany();
  placeBanner(main);
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
        void drawNew(view, held, outcome.match.query.type, router.href("records", {}), router.href("newRecord", {}), opened, (type) => router.href("newRecord", {}, { type }));
      }
      break;
    }
    case "editRecord": {
      const held = unlocked();
      if (held !== null) {
        const record = outcome.match.params.record;
        void drawEdit(view, held, record, router.href("records", {}), router.href("record", { record }), (saved) => go(router.href("record", { record: saved })));
      }
      break;
    }
    case "members":
      withHeld((held) => drawMembers(view, held, (account) => router.href("member", { account })));
      break;
    case "member": {
      const account = outcome.match.params.account;
      withHeld((held) => drawMember(view, held, account, router.href("members", {}), () => go(router.href("members", {}))));
      break;
    }
    case "groups": {
      const held = unlocked();
      if (held !== null) {
        void drawGroups(view, held, (group) => router.href("group", { group }));
      }
      break;
    }
    case "group": {
      const held = unlocked();
      if (held !== null) {
        void drawGroup(view, held, outcome.match.params.group, router.href("groups", {}), () => go(router.href("groups", {})));
      }
      break;
    }
    case "trail":
      withHeld((held) => drawTrail(view, held));
      break;
    case "recovery":
      withHeld((held) => drawRecovery(view, held));
      break;
    case "account":
      withHeld((held) => drawAccount(view, held));
      break;
    case "company":
      withHeld((held) => drawCompany(view, held));
      break;
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
    case "join": {
      // The link is single-use: it leaves the address bar and the history entry before anything else.
      window.history.replaceState(null, "", `${window.location.pathname}${router.href("join", {})}`);
      if (signedIn()) {
        go(router.href("records", {}));
        return;
      }
      join(view, outcome.match.query.token ?? "", () => go(router.href("records", {})));
      break;
    }
    case "unlock": {
      const next = outcome.match.query.next;
      if (isUnlocked()) {
        go(afterUnlock(next));
        return;
      }
      const afterOpening = (): void => {
        const held = unlocked();
        if (held !== null) {
          void checkNotices(held, () => main, router.href("recovery", {}));
        }
        go(afterUnlock(next));
      };
      void unlock(view, afterOpening, () => void signOut());
      break;
    }
  }
}

function mark(outcome: Outcome<Routes>): void {
  const name = outcome.kind === "matched" ? outcome.match.name : "";
  const section = sectionOf(name);
  for (const link of document.querySelectorAll<HTMLAnchorElement>("nav a[data-section]")) {
    if (link.dataset.section === section) {
      // The list itself is the page; a record, group or member under it is only in its section.
      link.setAttribute("aria-current", name === section ? "page" : "true");
    } else {
      link.removeAttribute("aria-current");
    }
  }
}

/** The unlocked company's name and who is signed in, in the sidebar; "Switch company" when the person is in others too. */
function showCompany(): void {
  const held = unlocked();
  const name = document.getElementById("company");
  const workspace = document.getElementById("workspace");
  const switcher = document.getElementById("switch-company");
  if (name !== null && workspace !== null) {
    name.textContent = held?.companyName ?? "";
    workspace.hidden = held === null;
  }
  const account = document.getElementById("account");
  const person = document.getElementById("account-name");
  const email = document.getElementById("account-email");
  if (account !== null && person !== null && email !== null) {
    person.textContent = held?.name ?? "";
    email.textContent = held?.email ?? "";
    account.hidden = held === null;
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
    dropBanner();
    void copy.clearNow();
    if (signedIn()) {
      go(`#/unlock?next=${encodeURIComponent(history.current().replace(/^#/, ""))}`);
    }
  });
}

const view = document.getElementById("view");
if (view !== null) {
  decorateShell();
  installMenu(window);
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
