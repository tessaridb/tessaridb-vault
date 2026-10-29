// Joining with an enrolment link an admin gave: the link is checked, the person adds the vault to their authenticator
// app, then chooses a sign-in password and proves the app with its first code. That signs them in; the unlock password
// comes next, on its own page. The link is single-use, so it leaves the address bar as soon as it is read.

import { call } from "./api.ts";
import { el, field, fill } from "./dom.ts";
import { groupedKey, joinFailure, passwordProblem, readAuthenticator, tokenFrom, type Authenticator } from "./join-model.ts";
import { refusal } from "./screen.ts";
import { keep } from "./session.ts";
import { card } from "./sign-in.ts";
import { readSession } from "./sign-in-model.ts";

/** The second step: the authenticator setup, the new sign-in password and the first code. */
function setUp(main: HTMLElement, token: string, made: Authenticator, done: () => void): void {
  const key = el("p", { class: "fingerprint", id: "setup-key" }, groupedKey(made.secret));
  const app = el("a", { href: made.uri }, "Open in your authenticator app");
  const password = field("new-password", "Sign-in password", { type: "password", autocomplete: "new-password", required: "" }, "8 to 256 characters, without your email address.");
  const again = field("password-again", "Type it again", { type: "password", autocomplete: "new-password", required: "" });
  const code = field(
    "code",
    "Code from your authenticator app",
    { inputmode: "numeric", autocomplete: "one-time-code", pattern: "[0-9]{6}", maxlength: "6", required: "", spellcheck: "false" },
    "Six digits, after you add the vault to the app.",
  );
  const submit = el("button", { type: "submit", class: "primary" }, "Join");
  const said = el("div", {});
  const view = card(
    "Set up your sign-in",
    "Add the vault to your authenticator app with this key, or open the link on this device.",
    el("div", { class: "field" }, el("p", { class: "field-label" }, "Setup key"), key, app),
    password.row,
    again.row,
    code.row,
    submit,
    said,
  );
  view.querySelector("form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const problem = passwordProblem(password.input.value, again.input.value);
    if (problem !== null) {
      password.input.setAttribute("aria-invalid", "true");
      fill(said, refusal(problem));
      password.input.focus();
      return;
    }
    submit.disabled = true;
    const answer = await call("POST", "/api/enrolment/complete", readSession, {
      body: { token, password: password.input.value, code: code.input.value.trim() },
    });
    submit.disabled = false;
    code.input.value = "";
    if (answer.ok) {
      password.input.value = "";
      again.input.value = "";
      keep(answer.value);
      done();
      return;
    }
    fill(said, refusal(joinFailure(answer)));
    (answer.code === "weak_password" ? password.input : code.input).focus();
  });
  fill(main, view);
  password.input.focus();
}

/** Draws joining into `main`, the link's token already filled in when the link carried one. */
export function join(main: HTMLElement, carried: string, done: () => void): void {
  const token = field("token", "Enrolment link or code", { autocomplete: "off", required: "", spellcheck: "false" }, "From the message your admin sent.");
  token.input.value = carried;
  const submit = el("button", { type: "submit", class: "primary" }, "Continue");
  const said = el("div", {});
  const view = card("Join TessariDB Vault", "Your admin added you. Set up your sign-in to start.", token.row, submit, said);
  view.querySelector("form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const value = tokenFrom(token.input.value);
    submit.disabled = true;
    const answer = await call("POST", "/api/enrolment/begin", readAuthenticator, { body: { token: value } });
    submit.disabled = false;
    if (answer.ok) {
      setUp(main, value, answer.value, done);
      return;
    }
    fill(said, refusal(joinFailure(answer)));
    token.input.focus();
  });
  fill(main, view);
  token.input.focus();
}
