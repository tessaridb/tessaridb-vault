// Signing in: work email and password, then a code from the person's authenticator app; or a passkey; or the
// organisation's identity provider when this server has one. The password and the code go to the server once and the
// fields are cleared as soon as the answer arrives; what the page keeps is the session token, in memory only.

import { call, type Failure } from "./api.ts";
import { decode, encode } from "./base64url.ts";
import { el, field, fill } from "./dom.ts";
import { heading, refusal } from "./screen.ts";
import { keep } from "./session.ts";
import {
  PROVIDER_START,
  isProviderReturn,
  readChallenge,
  readMethods,
  readPasskeyRequest,
  readSession,
  signInFailure,
  type PasskeyRequest,
} from "./sign-in-model.ts";

/** The card every step of signing in is drawn in. */
function card(title: string, lead: string, ...body: HTMLElement[]): HTMLElement {
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
    ),
  );
}

/** Asks the browser for a passkey of this server and sends the assertion; a refusal or a cancelled prompt is a failure. */
async function passkey(request: PasskeyRequest): Promise<Failure | string> {
  const challenge = decode(request.challenge);
  if (challenge === null) {
    return { ok: false, status: 0, code: "bad_answer", message: "" };
  }
  let credential: Credential | null;
  try {
    credential = await navigator.credentials.get({
      publicKey: { challenge, rpId: request.rpId, userVerification: "required" },
    });
  } catch {
    return { ok: false, status: 0, code: "passkey_refused", message: "" };
  }
  if (!(credential instanceof PublicKeyCredential) || !(credential.response instanceof AuthenticatorAssertionResponse)) {
    return { ok: false, status: 0, code: "passkey_refused", message: "" };
  }
  const response = credential.response;
  if (response.userHandle === null) {
    return { ok: false, status: 0, code: "passkey_refused", message: "" };
  }
  const answer = await call("POST", "/api/sign-in/passkey", readSession, {
    body: {
      credential_id: encode(new Uint8Array(credential.rawId)),
      client_data_json: encode(new Uint8Array(response.clientDataJSON)),
      authenticator_data: encode(new Uint8Array(response.authenticatorData)),
      signature: encode(new Uint8Array(response.signature)),
      user_handle: encode(new Uint8Array(response.userHandle)),
    },
  });
  return answer.ok ? answer.value : answer;
}

/** The code step, after the password was accepted. */
function codeStep(main: HTMLElement, challenge: string, done: () => void, restart: () => void): void {
  const code = field(
    "code",
    "Code from your authenticator app",
    { inputmode: "numeric", autocomplete: "one-time-code", pattern: "[0-9]{6}", maxlength: "6", required: "", spellcheck: "false" },
    "Six digits. A new one appears every 30 seconds.",
  );
  const submit = el("button", { type: "submit", class: "primary" }, "Sign in");
  const other = el("button", { type: "button", class: "quiet" }, "Use a different account");
  const said = el("div", {});
  const view = card("Enter your code", "Your password was right. One more step.", code.row, submit, other, said);
  other.addEventListener("click", restart);
  view.querySelector("form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    submit.disabled = true;
    const answer = await call("POST", "/api/sign-in/code", readSession, { body: { challenge, code: code.input.value.trim() } });
    code.input.value = "";
    submit.disabled = false;
    if (answer.ok) {
      keep(answer.value);
      done();
      return;
    }
    // A refused challenge cannot take another code: the person starts again from the password.
    if (answer.code === "sign_in_failed") {
      fill(said, refusal("This sign-in expired. Start again with your password."));
      other.textContent = "Start again";
      other.focus();
      return;
    }
    code.input.setAttribute("aria-invalid", "true");
    fill(said, refusal(signInFailure(answer)));
    code.input.focus();
  });
  fill(main, view);
  code.input.focus();
}

/** Draws signing in into `main`, saying `notice` first when there is one; `done` runs once a session exists. */
export function signIn(main: HTMLElement, done: () => void, notice: string | null = null): void {
  const restart = (): void => signIn(main, done);
  const email = field("email", "Work email", { type: "email", autocomplete: "username", required: "", spellcheck: "false" });
  const password = field("password", "Password", { type: "password", autocomplete: "current-password", required: "" });
  const submit = el("button", { type: "submit", class: "primary" }, "Continue");
  const others = el("div", { class: "other-ways" });
  const said = el("div", {}, notice === null ? null : refusal(notice));
  const view = card(
    "Sign in to TessariDB Vault",
    "Use your work email and password, then a code from your authenticator app.",
    email.row,
    password.row,
    submit,
    others,
    said,
  );
  view.querySelector("form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    submit.disabled = true;
    const answer = await call("POST", "/api/sign-in", readChallenge, {
      body: { email: email.input.value.trim(), password: password.input.value },
    });
    password.input.value = "";
    submit.disabled = false;
    if (answer.ok) {
      codeStep(main, answer.value, done, restart);
      return;
    }
    fill(said, refusal(signInFailure(answer)));
    password.input.focus();
  });
  fill(main, view);
  email.input.focus();
  void call("GET", "/api/sign-in/methods", readMethods).then((methods) => {
    if (!methods.ok) {
      return;
    }
    const offered: HTMLElement[] = [];
    if (methods.value.passkey && typeof PublicKeyCredential === "function") {
      const button = el("button", { type: "button" }, "Sign in with a passkey");
      button.addEventListener("click", async () => {
        button.disabled = true;
        const begun = await call("POST", "/api/sign-in/passkey/begin", readPasskeyRequest);
        const result = begun.ok ? await passkey(begun.value) : begun;
        button.disabled = false;
        if (typeof result === "string") {
          keep(result);
          done();
          return;
        }
        fill(said, refusal(signInFailure(result)));
      });
      offered.push(button);
    }
    if (methods.value.provider) {
      offered.push(el("a", { class: "button", href: PROVIDER_START }, "Sign in with your organisation"));
    }
    if (offered.length > 0) {
      fill(others, el("p", { class: "muted" }, "or"), ...offered);
    }
  });
}

/** When the identity provider has just sent the person back, finishes that sign-in: null when there was no return or it
 *  signed the person in, else what went wrong, for the sign-in page to say. */
export async function providerReturned(): Promise<string | null> {
  const search = window.location.search;
  if (!isProviderReturn(search)) {
    return null;
  }
  // The provider's code and state leave the address bar at once, whatever the answer.
  window.history.replaceState(null, "", `${window.location.pathname}${window.location.hash}`);
  const answer = await call("GET", `/api/sign-in/oidc/return${search}`, readSession);
  if (answer.ok) {
    keep(answer.value);
    return null;
  }
  return signInFailure(answer);
}
