// The account page: who is signed in and where, a way to add a passkey, which then signs this person in without a
// password or a code, and the person's passkeys, each removable at once (a lost device's passkey stops working). A
// passkey is made by the person's device; the vault keeps only its public key. Where the vault offers an organisation
// sign-in, the person links it here: the browser leaves for the provider and comes back signed out, the link made.

import { call, ignored, type Failure } from "../api.ts";
import { encode } from "../base64url.ts";
import { el, fill } from "../dom.ts";
import { heading, refusal } from "../screen.ts";
import { readLinkAddress, readMethods } from "../sign-in-model.ts";
import type { Unlocked } from "../vault.ts";
import { creationOptions, passkeyFailure, readPasskeys, readRegistrationOptions, type ListedPasskey } from "./passkey.ts";

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
  } catch (problem) {
    // The browser says InvalidStateError when this device already holds one of the person's passkeys.
    return problem instanceof DOMException && problem.name === "InvalidStateError" ? { ...cancelled, code: "passkey_registered" } : cancelled;
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

const when = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });

/** One passkey with the act that removes it, confirmed with the passkey named by when it was added. */
function passkeyRow(key: ListedPasskey, said: HTMLElement, again: (words: string) => void): HTMLElement {
  const added = when.format(new Date(key.created * 1000));
  const keep = el("button", { type: "button", autofocus: "" }, "Keep it");
  const go = el("button", { type: "button", class: "danger" }, "Remove the passkey");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": `remove-${key.credential}` },
    el("h2", { id: `remove-${key.credential}` }, `Remove the passkey added ${added}?`),
    el("p", {}, "It stops signing you in at once, on every device that holds it. Your password and code still work."),
    el("div", { class: "actions" }, keep, go),
  );
  keep.addEventListener("click", () => dialog.close());
  go.addEventListener("click", () => {
    dialog.close();
    fill(said, el("p", { class: "muted", role: "status" }, "Removing the passkey."));
    void call("DELETE", `/api/me/passkeys/${encodeURIComponent(key.credential)}`, ignored).then((answer) => (answer.ok ? again("The passkey is removed.") : fill(said, refusal(passkeyFailure(answer)))));
  });
  const open = el("button", { type: "button" }, "Remove");
  open.setAttribute("aria-label", `Remove the passkey added ${added}`);
  open.addEventListener("click", () => dialog.showModal());
  const detail = key.backupEligible ? "Can sync to your other devices" : "Kept on one device";
  return el("li", { class: "share" }, el("div", {}, el("span", { class: "share-name" }, `Added ${added}`), el("span", { class: "detail" }, detail)), open, dialog);
}

/** The act that makes a passkey on this device and registers it. */
function addButton(held: Unlocked, said: HTMLElement, again: (words: string) => void): HTMLElement {
  const add = el("button", { type: "button" }, "Add a passkey");
  add.addEventListener("click", () => {
    add.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Follow your device's prompt."));
    void addPasskey(held).then((failed) => {
      add.disabled = false;
      if (failed === null) {
        again("The passkey is added. Next time, sign in with it.");
        return;
      }
      fill(said, refusal(passkeyFailure(failed)));
    });
  });
  return add;
}

/** The section offering to link the organisation sign-in; leaving for the provider is the last thing it does. */
function providerSection(said: HTMLElement): HTMLElement {
  const link = el("button", { type: "button" }, "Link organisation sign-in");
  link.addEventListener("click", () => {
    link.disabled = true;
    void call("POST", "/api/me/identities/oidc", readLinkAddress).then((answer) => {
      if (answer.ok) {
        window.location.assign(answer.value);
        return;
      }
      link.disabled = false;
      fill(said, refusal(answer.code === "network" ? "The vault didn't answer. Check your connection, then try again." : "The vault couldn't begin the link. Try again in a minute."));
    });
  });
  return el(
    "section",
    { class: "record-section", "aria-labelledby": "provider" },
    el("h2", { id: "provider" }, "Organisation sign-in"),
    el("p", { class: "muted" }, "Link your account to your organisation's sign-in, then use it instead of your password and code. You'll sign in once more after linking."),
    link,
  );
}

/** Draws the account page into `main`; `notice` is said once at its top. */
export async function drawAccount(main: HTMLElement, held: Unlocked, notice: string | null = null): Promise<void> {
  const title = heading("Account");
  const head = el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, `${held.name} · ${held.email} · ${held.companyName}`)));
  const said = el("div", { "aria-live": "polite" }, notice === null ? null : el("p", { class: "muted", role: "status" }, notice));
  const again = (words: string): void => void drawAccount(main, held, words);
  const passkeys = el("section", { class: "record-section", "aria-labelledby": "passkeys" }, el("h2", { id: "passkeys" }, "Passkeys"), el("p", { class: "muted", role: "status" }, "Checking what this vault offers."));
  const provider = el("div", {});
  fill(main, head, said, passkeys, provider);
  title.focus();
  const methods = await call("GET", "/api/sign-in/methods", readMethods);
  if (methods.ok && methods.value.provider) {
    fill(provider, providerSection(said));
  }
  if (!methods.ok || !methods.value.passkey) {
    fill(passkeys, el("h2", { id: "passkeys" }, "Passkeys"), el("p", { class: "muted" }, methods.ok ? "This vault doesn't offer passkeys." : "The vault didn't say whether it offers passkeys. Reload the page to try again."));
    return;
  }
  // A browser that can't make a passkey can still remove one, such as a lost device's.
  const makes = typeof PublicKeyCredential === "function";
  const add = makes ? addButton(held, said, again) : el("p", { class: "muted" }, "This browser can't make passkeys.");
  const listed = await call("GET", "/api/me/passkeys", readPasskeys);
  const yours = !listed.ok
    ? refusal(passkeyFailure(listed))
    : listed.value.length === 0
      ? el("p", { class: "muted" }, "You have no passkeys yet.")
      : el("ul", { class: "shares" }, ...listed.value.map((key) => passkeyRow(key, said, again)));
  fill(passkeys, el("h2", { id: "passkeys" }, "Passkeys"), el("p", { class: "muted" }, "A passkey signs you in with this device's lock instead of your password and a code. Remove one you no longer hold, such as a lost device's."), yours, add);
}
