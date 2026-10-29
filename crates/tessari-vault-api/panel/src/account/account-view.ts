// The account page: who is signed in and where, and a way to add a passkey, which then signs this person in without a
// password or a code. A passkey is made by the person's device; the vault keeps only its public key.

import { call, ignored, type Failure } from "../api.ts";
import { encode } from "../base64url.ts";
import { el, fill } from "../dom.ts";
import { heading, refusal } from "../screen.ts";
import { readMethods } from "../sign-in-model.ts";
import type { Unlocked } from "../vault.ts";
import { creationOptions, passkeyFailure, readRegistrationOptions } from "./passkey.ts";

const cancelled: Failure = { ok: false, status: 0, code: "cancelled", message: "" };

/** Makes a passkey on this device and registers it; null once registered, else what went wrong. */
async function addPasskey(held: Unlocked): Promise<Failure | null> {
  const begun = await call("POST", "/api/me/passkeys/begin", readRegistrationOptions);
  if (!begun.ok) {
    return begun;
  }
  let credential: Credential | null;
  try {
    credential = await navigator.credentials.create({ publicKey: creationOptions(begun.value, held.name) });
  } catch {
    return cancelled;
  }
  if (!(credential instanceof PublicKeyCredential) || !(credential.response instanceof AuthenticatorAttestationResponse)) {
    return cancelled;
  }
  const kept = await call("POST", "/api/me/passkeys", ignored, {
    body: {
      client_data_json: encode(new Uint8Array(credential.response.clientDataJSON)),
      attestation_object: encode(new Uint8Array(credential.response.attestationObject)),
    },
  });
  return kept.ok ? null : kept;
}

/** Draws the account page into `main`. */
export async function drawAccount(main: HTMLElement, held: Unlocked): Promise<void> {
  const title = heading("Account");
  const head = el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, `${held.name} · ${held.email} · ${held.companyName}`)));
  const said = el("div", { "aria-live": "polite" });
  const passkeys = el("section", { class: "record-section", "aria-labelledby": "passkeys" }, el("h2", { id: "passkeys" }, "Passkeys"), el("p", { class: "muted", role: "status" }, "Checking what this vault offers."));
  fill(main, head, said, passkeys);
  title.focus();
  const methods = await call("GET", "/api/sign-in/methods", readMethods);
  if (!methods.ok || !methods.value.passkey || typeof PublicKeyCredential !== "function") {
    fill(passkeys, el("h2", { id: "passkeys" }, "Passkeys"), el("p", { class: "muted" }, methods.ok && !methods.value.passkey ? "This vault doesn't offer passkeys." : "This browser can't make passkeys."));
    return;
  }
  const add = el("button", { type: "button" }, "Add a passkey");
  add.addEventListener("click", () => {
    add.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Follow your device's prompt."));
    void addPasskey(held).then((failed) => {
      add.disabled = false;
      fill(said, failed === null ? el("p", { class: "muted", role: "status" }, "The passkey is added. Next time, sign in with it.") : refusal(passkeyFailure(failed)));
    });
  });
  fill(passkeys, el("h2", { id: "passkeys" }, "Passkeys"), el("p", { class: "muted" }, "A passkey signs you in with this device's lock instead of your password and a code."), add);
}
