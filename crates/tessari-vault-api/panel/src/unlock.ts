// Unlocking: the unlock password opens this person's keys for one company, here in the page; or, the first time, sets
// that password and makes their keys. The password never leaves the browser and is not the sign-in password, so nobody
// at the server can reset it; a company's owners can restore access through recovery.

import { call, type Failure } from "./api.ts";
import { VaultCryptoError } from "./crypto/errors.ts";
import { createKeySet, openKeySet, type StoredKeySet } from "./crypto/key-set.ts";
import { el, field, fill } from "./dom.ts";
import { heading, refusal } from "./screen.ts";
import {
  keySetBody,
  newPasswordProblem,
  readCompanies,
  readKept,
  readMe,
  readOwnKeys,
  unlockable,
  type Company,
  type Me,
} from "./unlock-model.ts";
import { hold } from "./vault.ts";

let module: Promise<WebAssembly.Module> | null = null;

/** The vault's Argon2id module, fetched once from this server. */
function cryptoModule(): Promise<WebAssembly.Module> {
  module ??= WebAssembly.compileStreaming(fetch("/vault-crypto.wasm"));
  return module;
}

/** While locked the sidebar is hidden, so every unlocking card offers signing out itself. */
let signOutHere: () => void = () => {};

function card(title: string, lead: string, ...body: Array<HTMLElement | null>): HTMLElement {
  const leave = el("button", { type: "button", class: "quiet" }, "Sign out");
  leave.addEventListener("click", () => signOutHere());
  return el(
    "div",
    { class: "gate" },
    el(
      "form",
      { class: "card", novalidate: "" },
      el("img", { class: "logo", src: "/favicon.svg", alt: "", width: "48", height: "48" }),
      heading(title),
      el("p", { class: "muted" }, lead),
      ...body,
      leave,
    ),
  );
}

/** What went wrong while unlocking, in words. */
function unlockFailure(problem: unknown): string {
  if (problem instanceof VaultCryptoError && problem.reason === "unauthentic") {
    return "That unlock password doesn't open your keys. Check it and try again; it is not your sign-in password.";
  }
  if (problem instanceof VaultCryptoError && problem.reason === "kdf-refused") {
    return "Your keys were made with settings this page can't run. Tell your admin.";
  }
  return "This page couldn't open your keys. Reload the page and try again.";
}

const loadFailure = (failure: Failure): string =>
  failure.code === "network" ? "The vault didn't answer. Check your connection, then try again." : "The vault couldn't load your keys. Try again in a minute.";

/** A company choice when the person belongs to several: radios, so the arrow keys move between them. */
function companyChoice(companies: readonly Company[]): { readonly row: HTMLElement | null; readonly chosen: () => Company } {
  const first = companies[0];
  if (first === undefined) {
    throw new RangeError("no company to choose");
  }
  if (companies.length === 1) {
    return { row: null, chosen: () => first };
  }
  const inputs = companies.map((company, at) => {
    const input = el("input", { type: "radio", name: "company", id: `company-${at}`, value: company.id });
    input.checked = at === 0;
    return input;
  });
  const row = el(
    "fieldset",
    { class: "field choices" },
    el("legend", {}, "Company"),
    ...inputs.flatMap((input, at) => [el("div", { class: "choice" }, input, el("label", { for: input.id }, companies[at]?.name ?? ""))]),
  );
  return { row, chosen: () => companies.find((company) => company.id === inputs.find((input) => input.checked)?.value) ?? first };
}

/** The form that opens an existing key set. */
function unlockForm(main: HTMLElement, me: Me, company: Company, stored: StoredKeySet, choice: HTMLElement | null, done: () => void): void {
  const password = field("unlock-password", "Unlock password", { type: "password", autocomplete: "current-password", required: "" });
  const submit = el("button", { type: "submit", class: "primary" }, "Unlock");
  const said = el("div", { "aria-live": "polite" });
  const view = card("Unlock your vault", `Your records in ${company.name} open here, in this browser.`, choice, password.row, submit, said);
  view.querySelector("form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    submit.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Opening your keys. This takes a second."));
    try {
      const keys = await openKeySet(await cryptoModule(), password.input.value, me.account, company.id, stored);
      password.input.value = "";
      hold({ company: company.id, account: me.account, keys });
      done();
    } catch (problem) {
      password.input.value = "";
      password.input.setAttribute("aria-invalid", "true");
      fill(said, refusal(unlockFailure(problem)));
      password.input.focus();
    } finally {
      submit.disabled = false;
    }
  });
  fill(main, view);
  password.input.focus();
}

/** The form that sets the unlock password the first time and makes the person's keys for `company`. */
function firstUnlockForm(main: HTMLElement, me: Me, company: Company, choice: HTMLElement | null, done: () => void): void {
  const password = field(
    "new-unlock-password",
    "New unlock password",
    { type: "password", autocomplete: "new-password", required: "" },
    "At least 12 characters. Not your sign-in password: the vault can't reset this one.",
  );
  const repeated = field("repeat-unlock-password", "Type it again", { type: "password", autocomplete: "new-password", required: "" });
  const submit = el("button", { type: "submit", class: "primary" }, "Set unlock password");
  const said = el("div", { "aria-live": "polite" });
  const view = card(
    "Set your unlock password",
    `Your records in ${company.name} are encrypted with keys this browser makes now. The unlock password opens them.`,
    choice,
    password.row,
    repeated.row,
    submit,
    said,
  );
  view.querySelector("form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const problem = newPasswordProblem(password.input.value, repeated.input.value, me.email);
    if (problem !== null) {
      fill(said, refusal(problem));
      password.input.focus();
      return;
    }
    submit.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Making your keys. This takes a few seconds."));
    try {
      const made = await createKeySet(await cryptoModule(), password.input.value, me.account, company.id);
      const answer = await call("PUT", `/api/companies/${encodeURIComponent(company.id)}/keys/me`, readKept(made.stored.keyId), {
        body: keySetBody(made.stored),
      });
      if (!answer.ok) {
        fill(said, refusal(loadFailure(answer)));
        return;
      }
      password.input.value = "";
      repeated.input.value = "";
      hold({ company: company.id, account: me.account, keys: made.open });
      done();
    } catch (problemMaking) {
      fill(said, refusal(unlockFailure(problemMaking)));
    } finally {
      submit.disabled = false;
    }
  });
  fill(main, view);
  password.input.focus();
}

/** Draws unlocking into `main`; `done` runs once a company's keys are open, `signOut` when the person leaves instead. */
export async function unlock(main: HTMLElement, done: () => void, signOut: () => void): Promise<void> {
  signOutHere = signOut;
  fill(main, card("Unlock your vault", "Loading your companies."));
  const [me, companies] = await Promise.all([call("GET", "/api/me", readMe), call("GET", "/api/companies", readCompanies)]);
  if (!me.ok) {
    fill(main, card("Unlock your vault", "", refusal(loadFailure(me))));
    return;
  }
  if (!companies.ok) {
    fill(main, card("Unlock your vault", "", refusal(loadFailure(companies))));
    return;
  }
  const open = unlockable(companies.value);
  if (open.length === 0) {
    fill(
      main,
      card(
        "No company yet",
        me.value.platformAdmin
          ? "You're a platform admin and not a member of any company, so there are no records to unlock."
          : "You aren't a member of an active company yet. Ask your company's admin to add you.",
      ),
    );
    return;
  }
  const choice = companyChoice(open);
  const draw = async (): Promise<void> => {
    const company = choice.chosen();
    const keys = await call("GET", `/api/companies/${encodeURIComponent(company.id)}/keys/me`, readOwnKeys);
    if (keys.ok) {
      unlockForm(main, me.value, company, keys.value, choice.row, done);
    } else if (keys.status === 404) {
      firstUnlockForm(main, me.value, company, choice.row, done);
    } else {
      fill(main, card("Unlock your vault", "", choice.row, refusal(loadFailure(keys))));
    }
  };
  choice.row?.addEventListener("change", () => void draw());
  await draw();
}
