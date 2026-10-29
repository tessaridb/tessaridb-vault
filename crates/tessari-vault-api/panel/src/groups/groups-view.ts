// The groups page: the company, its departments and the teams in them, each with whether it has a key yet, a form to
// make a department or a team, and, for a group with no key, a way to key it. Keying is done here: the key pair is
// made in this browser, a chain statement vouches for it, and only the wraps leave the page.

import { call, ignored } from "../api.ts";
import { readChain } from "../chain-model.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { verifyChain, type Statement, type Vouched } from "../crypto/statements.ts";
import { readGroupKey } from "../records-model.ts";
import { el, fill } from "../dom.ts";
import { statementBody } from "../members/members-model.ts";
import { vouchFailure, vouchRefused } from "../members/members-words.ts";
import { CHAIN_REFUSED, loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { groupKeyBody, newGroupKey, pendingWraps, wrapsBody } from "./group-key.ts";
import { labelled, membersControl } from "./group-members-view.ts";
import { giveButton, keyButton } from "./group-words.ts";
import { createFailure, groupTree, readGroupRows, readPending, readRotationDue, type TreeRow } from "./groups-model.ts";

const KIND_WORDS = { company: "Company", department: "Department", team: "Team" } as const;

/** Draws the groups page into `main`; `notice` is said at its top, as after a change the chain had moved past. */
export async function drawGroups(main: HTMLElement, held: Unlocked, notice: string | null = null): Promise<void> {
  const title = heading("Departments and teams");
  const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Loading the groups."));
  fill(main, el("div", { class: "page-head" }, title), said);
  const path = companyPath(held);
  const [groups, chain] = await Promise.all([call("GET", `${path}/groups`, readGroupRows), call("GET", `${path}/statements`, readChain)]);
  if (!groups.ok || !chain.ok) {
    const failed = !groups.ok ? groups : chain;
    if (!failed.ok) {
      fill(said, refusal(loadFailure(failed)));
    }
    return;
  }
  const vouched = await verifyChain(held.company, held.keys.ownerSigningPublic, chain.value.statements).catch(() => null);
  if (vouched === null) {
    fill(said, refusal(CHAIN_REFUSED));
    return;
  }
  const again = (words: string | null = null): void => void drawGroups(main, held, words);
  const tree = groupTree({ id: held.company, name: held.companyName }, groups.value);
  // Only an owner or an admin is told who is waiting; anyone else is refused, and sees no count.
  const waiting = new Map(
    await Promise.all(
      tree
        .filter((row) => vouched.groupKeyOf(row.id) !== undefined)
        .map((row) => call("GET", `${path}/groups/${encodeURIComponent(row.id)}/key/pending`, readPending).then((answer) => [row.id, answer.ok ? answer.value.length : 0] as const)),
    ),
  );
  // A key due for rotation is made anew rather than handed on: whoever left may still hold the old one.
  const due = new Set(
    (
      await Promise.all(
        tree
          .filter((row) => vouched.groupKeyOf(row.id) !== undefined)
          .map((row) => call("GET", `${path}/groups/${encodeURIComponent(row.id)}/key`, readRotationDue).then((answer) => (answer.ok && answer.value === true ? row.id : null))),
      )
    ).filter((id) => id !== null),
  );
  const items = tree.map((row) => groupItem(held, row, vouched, due.has(row.id) ? null : (waiting.get(row.id) ?? null), due.has(row.id), chain.value.statements, again));
  fill(
    main,
    el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "A record shared with a group opens for everyone in it. A group needs a key before anything can be shared with it."))),
    notice === null ? null : refusal(notice),
    el("section", { class: "record-section", "aria-labelledby": "all-groups" }, el("h2", { id: "all-groups" }, "Groups"), el("ul", { class: "shares" }, ...items)),
    createForm(held, tree, again),
  );
  title.focus();
}

/** One group's line: its name, kind and key state, "Key this group" while it has none, and access for who waits. */
function groupItem(held: Unlocked, row: TreeRow, vouched: Vouched, waiting: number | null, rotate: boolean, chain: readonly Statement[], again: (words?: string | null) => void): HTMLElement {
  const said = el("div", { "aria-live": "polite" });
  const keyed = vouched.groupKeyOf(row.id) !== undefined;
  const give = waiting === null || waiting === 0 ? null : labelled(giveButton(row.name, waiting));
  give?.addEventListener("click", () => {
    give.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Giving the group's key to the members waiting."));
    void giveAccess(held, row.id, vouched).then((words) => {
      give.disabled = false;
      if (words === null) {
        again();
        return;
      }
      fill(said, refusal(words));
    });
  });
  const key = keyed && !rotate ? null : labelled(keyButton(row.name, rotate));
  key?.addEventListener("click", () => {
    key.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Making the group's key."));
    void keyGroup(held, row.id, chain).then((words) => {
      key.disabled = false;
      if (words === null) {
        again();
        return;
      }
      fill(said, refusal(words));
    });
  });
  return el(
    "li",
    { class: "share group", "data-depth": String(row.depth) },
    el("div", {}, el("span", { class: "share-name" }, row.name), el("span", { class: "detail" }, KIND_WORDS[row.kind])),
    el("span", { class: `state ${keyed && !rotate ? "vouched" : "waiting"}` }, rotate ? "Needs a new key" : keyed ? "Has a key" : "No key yet"),
    el("div", { class: "row-actions" }, key ?? give, ...(row.kind === "company" ? [] : membersControl(held, row, () => again()))),
    said,
  );
}

/** Keys group `id`: a new pair vouched after `chain`, wrapped to the members waiting; null once kept, else why not. */
async function keyGroup(held: Unlocked, id: string, chain: readonly Statement[]): Promise<string | null> {
  const path = `${companyPath(held)}/groups/${encodeURIComponent(id)}/key`;
  const pending = await call("GET", `${path}/pending`, readPending);
  if (!pending.ok) {
    return pending.status === 403 ? "Only an owner or an admin can key a group." : loadFailure(pending);
  }
  const owner = held.keys.ownerSigningPublic;
  const signer = { privateKey: held.keys.signingPrivate, publicBytes: held.keys.signingPublic };
  let made;
  try {
    made = await newGroupKey(held.company, id, signer, owner, chain, await verifyChain(held.company, owner, chain), pending.value);
  } catch (problem) {
    return vouchRefused(problem instanceof VaultCryptoError ? problem.reason : "chain-broken");
  }
  const stated = await call("POST", `${companyPath(held)}/statements`, ignored, { body: statementBody(chain.length + 1, made.statement) });
  if (!stated.ok) {
    return vouchFailure(stated.status, stated.code, loadFailure(stated));
  }
  const kept = await call("PUT", path, ignored, { body: groupKeyBody(made) });
  return kept.ok ? null : loadFailure(kept);
}

/** Wraps group `id`'s key, opened from the caller's own wrap, to the members waiting; null once kept, else why not. */
async function giveAccess(held: Unlocked, id: string, vouched: Vouched): Promise<string | null> {
  const path = `${companyPath(held)}/groups/${encodeURIComponent(id)}/key`;
  const [listed, pending] = await Promise.all([call("GET", path, readGroupKey), call("GET", `${path}/pending`, readPending)]);
  if (!listed.ok || !pending.ok) {
    const failed = !listed.ok ? listed : pending;
    return failed.ok ? null : loadFailure(failed);
  }
  if (listed.value.wrap === null) {
    return "You don't hold this group's key. Ask someone in the group to give access.";
  }
  let given;
  try {
    given = await pendingWraps(held.company, id, held.keys, vouched, listed.value, pending.value);
  } catch (problem) {
    return vouchRefused(problem instanceof VaultCryptoError ? problem.reason : "chain-broken");
  }
  if (given.wraps.length === 0) {
    return "Nobody has vouched for the waiting members' keys yet. Vouch for them on the Members page first.";
  }
  const kept = await call("POST", `${path}/wraps`, ignored, { body: wrapsBody(given) });
  return kept.ok ? null : loadFailure(kept);
}

/** The form that makes a department, in the company, or a team, in the company or a department. */
function createForm(held: Unlocked, tree: readonly TreeRow[], again: (words?: string | null) => void): HTMLElement {
  let kind: "department" | "team" = "department";
  let parent = held.company;
  const name = el("input", { id: "group-name", type: "text", autocomplete: "off" });
  const kinds = (["department", "team"] as const).map((each) => el("button", { type: "button", class: "chip", "aria-pressed": String(each === kind) }, KIND_WORDS[each]));
  const places = tree.filter((row) => row.kind !== "team");
  const parents = el("fieldset", { class: "choices" }, el("legend", {}, "Sits in"), el("div", { class: "targets" }, ...places.map((row) => placeRow(row, row.id === parent, () => (parent = row.id)))));
  parents.hidden = true;
  kinds.forEach((button, at) =>
    button.addEventListener("click", () => {
      kind = at === 0 ? "department" : "team";
      kinds.forEach((each, other) => each.setAttribute("aria-pressed", String(other === at)));
      parents.hidden = kind === "department";
    }),
  );
  const submit = el("button", { type: "submit", class: "primary" }, "Make group");
  const said = el("div", { "aria-live": "polite" });
  const form = el(
    "form",
    { class: "record-section", novalidate: "" },
    el("h2", {}, "New department or team"),
    el("div", { class: "chips", role: "group", "aria-label": "Kind" }, ...kinds),
    el("div", { class: "field" }, el("label", { for: name.id }, "Name"), name),
    parents,
    submit,
    said,
  );
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    if (name.value.trim() === "") {
      fill(said, refusal("Give the group a name."));
      name.focus();
      return;
    }
    submit.disabled = true;
    // The company is where a group sits when none is named; only a team in a department names its parent.
    const inside = kind === "team" && parent !== held.company ? { parent } : {};
    const body = { kind, name: name.value.trim(), ...inside };
    void call("POST", `${companyPath(held)}/groups`, ignored, { body }).then((answer) => {
      submit.disabled = false;
      if (answer.ok) {
        again();
        return;
      }
      fill(said, refusal(createFailure(answer.code, loadFailure(answer))));
    });
  });
  return form;
}

function placeRow(row: TreeRow, checked: boolean, choose: () => void): HTMLElement {
  const id = `group-parent-${row.id}`;
  const radio = el("input", { id, type: "radio", name: "group-parent", ...(checked ? { checked: "" } : {}) });
  radio.addEventListener("change", choose);
  return el("div", { class: "choice" }, radio, el("label", { for: id }, row.name, el("span", { class: "detail" }, KIND_WORDS[row.kind])));
}
