// Choosing who a new record is for while it is made, and sharing it with them once it exists. The record key is
// wrapped here to the group's key, only when the company's chain vouches for it; only the wrap is sent.

import { call, ignored } from "../api.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { el } from "../dom.ts";
import { readGroupList, type ShareTarget } from "./share-targets.ts";
import { readOpened } from "../records-model.ts";
import type { Unlocked } from "../vault.ts";
import { audienceChoices } from "./audience.ts";
import { CHAIN_REFUSED, loadFailure } from "./list-view.ts";
import { Opener, companyPath, vouchedChain } from "./load.ts";
import { shareBody } from "./share-record.ts";
import { sealFailure, shareFailure } from "./share-words.ts";
import { loadGroupKeys } from "./sharing.ts";

/** The groups `held`'s company can make a record for, or the words for why they could not be listed. */
export async function loadAudience(held: Unlocked): Promise<{ readonly ok: true; readonly choices: readonly ShareTarget[] } | { readonly ok: false; readonly words: string }> {
  const groups = await call("GET", `${companyPath(held)}/groups`, readGroupList);
  if (!groups.ok) {
    return { ok: false, words: loadFailure(groups) };
  }
  const company = { id: held.company, name: held.companyName };
  const keys = await loadGroupKeys(held, [{ id: company.id, kind: "company", name: company.name }, ...groups.value]);
  return keys.ok ? { ok: true, choices: audienceChoices(company, groups.value, keys.keys) } : { ok: false, words: loadFailure(keys) };
}

/** The "Who can open it" choice: only the author, or one of `choices`; a group with no key yet cannot be chosen. */
export function audiencePicker(choices: readonly ShareTarget[]): { readonly section: HTMLElement; readonly chosen: () => ShareTarget | null } {
  let chosen: ShareTarget | null = null;
  const row = (id: string, label: string, detail: string, choose: () => void, checked: boolean, disabled: boolean): HTMLElement => {
    const radio = el("input", { id, type: "radio", name: "record-audience", ...(checked ? { checked: "" } : {}) });
    radio.disabled = disabled;
    radio.addEventListener("change", choose);
    return el("div", { class: "choice" }, radio, el("label", { for: id }, label, el("span", { class: "detail" }, detail)));
  };
  const rows = [
    row("audience-only-me", "Only you", "No one else until you share it", () => (chosen = null), true, false),
    ...choices.map((choice) =>
      row(
        `audience-${choice.id}`,
        choice.label,
        choice.recipient === null ? `${choice.detail} · ${choice.missing}` : choice.detail,
        () => (chosen = choice),
        false,
        choice.recipient === null,
      ),
    ),
  ];
  const section = el("fieldset", { class: "choices" }, el("legend", {}, "Who can open it"), el("div", { class: "targets" }, ...rows));
  return { section, chosen: () => chosen };
}

/** Shares the just-made record `id` with `to` for reading; null once it is shared, else the words for why not. */
export async function shareCreated(held: Unlocked, id: string, to: NonNullable<ShareTarget["recipient"]>): Promise<string | null> {
  const path = companyPath(held);
  const [chain, answer] = await Promise.all([vouchedChain(held), call("GET", `${path}/records/${encodeURIComponent(id)}`, readOpened)]);
  if (!chain.ok) {
    return "problem" in chain ? CHAIN_REFUSED : loadFailure(chain);
  }
  if (!answer.ok) {
    return loadFailure(answer);
  }
  let body: Readonly<Record<string, unknown>>;
  try {
    const opener = new Opener(held, chain.vouched);
    body = await shareBody(held.company, chain.vouched, await opener.keyFor(answer.value), answer.value, to, "read");
  } catch (problem) {
    return sealFailure(problem instanceof VaultCryptoError ? problem.reason : "malformed");
  }
  const shared = await call("POST", `${path}/records/${encodeURIComponent(id)}/shares`, ignored, { body });
  return shared.ok ? null : shareFailure(shared.status, shared.code, loadFailure(shared));
}
