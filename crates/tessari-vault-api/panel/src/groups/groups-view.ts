// The groups page: the company, its departments and the teams in them, each with whether it has a key yet, a form to
// make a department or a team, and, for a group with no key, a way to key it. Keying is done here: the key pair is
// made in this browser, a chain statement vouches for it, and only the wraps leave the page.

import { call, ignored } from "../api.ts";
import { readChain } from "../chain-model.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { verifyChain, type Statement } from "../crypto/statements.ts";
import { el, fill } from "../dom.ts";
import { statementBody } from "../members/members-model.ts";
import { vouchFailure, vouchRefused } from "../members/members-words.ts";
import { CHAIN_REFUSED, loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { groupKeyBody, newGroupKey } from "./group-key.ts";
import { createFailure, groupTree, readGroupRows, readPending, type TreeRow } from "./groups-model.ts";

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
  const items = tree.map((row) => groupItem(held, row, vouched.groupKeyOf(row.id) !== undefined, chain.value.statements, again));
  fill(
    main,
    el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "A record shared with a group opens for everyone in it. A group needs a key before anything can be shared with it."))),
    notice === null ? null : refusal(notice),
    el("section", { class: "record-section", "aria-labelledby": "all-groups" }, el("h2", { id: "all-groups" }, "Groups"), el("ul", { class: "shares" }, ...items)),
    createForm(held, tree, again),
  );
  title.focus();
}

/** One group's line: its name, kind and key state, and "Key this group" while it has none. */
function groupItem(held: Unlocked, row: TreeRow, keyed: boolean, chain: readonly Statement[], again: (words?: string | null) => void): HTMLElement {
  const said = el("div", { "aria-live": "polite" });
  const key = keyed ? null : el("button", { type: "button", "aria-label": `Key ${row.name}` }, "Key this group");
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
    el("span", { class: `state ${keyed ? "vouched" : "waiting"}` }, keyed ? "Has a key" : "No key yet"),
    key ?? el("span", {}),
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
