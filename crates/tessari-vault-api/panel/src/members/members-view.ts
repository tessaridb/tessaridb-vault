// The members page: everyone in the company with their key's state and fingerprint, the person's own fingerprint to
// read out to an admin, and for owners and admins a way to vouch for a key that is waiting and to add someone. Vouching is a statement the
// browser signs and checks against the whole chain before the server keeps it.

import { call, ignored } from "../api.ts";
import { readChain } from "../chain-model.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { verifyChain } from "../crypto/statements.ts";
import { el, fill } from "../dom.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { CHAIN_REFUSED, loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import { readDirectory } from "../records/shares-model.ts";
import { memberRows, statementBody, vouchStatement, type MemberRow } from "./members-model.ts";
import { keyStateWords, vouchFailure, vouchRefused } from "./members-words.ts";
import { addMemberForm, invitedBlock, type Invited } from "./add-member.ts";

const ROLE_WORDS = { owner: "Owner", admin: "Admin", member: "Member" } as const;

/** Draws the members page into `main`; `notice` is said at its top, as after a vouch the chain had moved past, and
 *  `invited` shows the join link of someone just added. */
export async function drawMembers(main: HTMLElement, held: Unlocked, notice: string | null = null, invited: Invited | null = null): Promise<void> {
  const title = heading("Members");
  const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Loading the members."));
  fill(main, el("div", { class: "page-head" }, title), said);
  const path = companyPath(held);
  const [members, directory, chain] = await Promise.all([
    call("GET", `${path}/members`, readMemberList),
    call("GET", `${path}/keys`, readDirectory),
    call("GET", `${path}/statements`, readChain),
  ]);
  if (!members.ok || !directory.ok || !chain.ok) {
    const failed = !members.ok ? members : !directory.ok ? directory : chain;
    if (!failed.ok) {
      fill(said, refusal(loadFailure(failed)));
    }
    return;
  }
  const owner = held.keys.ownerSigningPublic;
  const vouched = await verifyChain(held.company, owner, chain.value.statements).catch(() => null);
  if (vouched === null) {
    fill(said, refusal(CHAIN_REFUSED));
    return;
  }
  const rows = await memberRows(members.value, directory.value, vouched);
  const me = rows.find((row) => row.account === held.account);
  const mayVouch = me?.role === "owner" || me?.role === "admin";
  const again = (words: string | null = null): void => void drawMembers(main, held, words);

  const fingerprint = el("p", { class: "fingerprint" });
  const dialogSaid = el("div", { "aria-live": "polite" });
  const dialogTitle = el("h2", { id: "vouch-title" });
  const notNow = el("button", { type: "button", autofocus: "" }, "Not now");
  const confirm = el("button", { type: "button", class: "primary" }, "Vouch for this key");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": dialogTitle.id },
    dialogTitle,
    el("p", {}, "Ask them to read out the fingerprint on their own Members page, by phone or in person. Vouch only if every group matches."),
    fingerprint,
    dialogSaid,
    el("div", { class: "actions" }, notNow, confirm),
  );
  let chosen: MemberRow | null = null;
  notNow.addEventListener("click", () => dialog.close());
  confirm.addEventListener("click", () => {
    const row = chosen;
    const key = row === null ? undefined : directory.value.get(row.account);
    if (row === null || key === undefined) {
      return;
    }
    confirm.disabled = true;
    fill(dialogSaid, el("p", { class: "muted", role: "status" }, "Vouching for the key."));
    const signer = { privateKey: held.keys.signingPrivate, publicBytes: held.keys.signingPublic };
    void vouchStatement(held.company, owner, chain.value.statements, signer, row.account, key)
      .then(async (statement) => {
        const answer = await call("POST", `${path}/statements`, ignored, { body: statementBody(chain.value.statements.length + 1, statement) });
        confirm.disabled = false;
        if (answer.ok) {
          dialog.close();
          again();
          return;
        }
        const words = vouchFailure(answer.status, answer.code, loadFailure(answer));
        if (answer.code === "stale_chain") {
          dialog.close();
          again(words);
          return;
        }
        fill(dialogSaid, refusal(words));
      })
      .catch((problem: unknown) => {
        confirm.disabled = false;
        fill(dialogSaid, refusal(vouchRefused(problem instanceof VaultCryptoError ? problem.reason : "chain-broken")));
      });
  });

  const items = rows.map((row) => {
    const vouch = mayVouch && !row.suspended && row.state === "waiting" ? el("button", { type: "button", "aria-label": `Vouch for ${row.label}'s key` }, "Vouch") : null;
    vouch?.addEventListener("click", () => {
      chosen = row;
      fill(dialogTitle, `Vouch for \u201C${row.label}\u201D's key?`);
      fill(fingerprint, row.fingerprint ?? "");
      fill(dialogSaid, null);
      dialog.showModal();
    });
    return el(
      "li",
      { class: "share member" },
      el("div", {}, el("span", { class: "share-name" }, row.account === held.account ? `${row.label} (you)` : row.label), el("span", { class: "detail" }, row.detail)),
      el("span", { class: "muted" }, ROLE_WORDS[row.role]),
      row.suspended ? el("span", { class: "state" }, "Suspended") : el("span", { class: `state ${row.state}` }, keyStateWords(row.state)),
      vouch ?? el("span", {}),
    );
  });
  const own = me?.fingerprint ?? null;
  fill(
    main,
    el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "Records are shared only with keys an owner or admin has vouched for, after comparing the key's fingerprint with its holder."))),
    notice === null ? null : refusal(notice),
    invited === null ? null : invitedBlock(invited),
    own === null ? null : el("section", { class: "record-section", "aria-labelledby": "own-key" }, el("h2", { id: "own-key" }, "Your key's fingerprint"), el("p", { class: "fingerprint" }, own)),
    el("section", { class: "record-section", "aria-labelledby": "everyone" }, el("h2", { id: "everyone" }, "Everyone in the company"), el("ul", { class: "shares" }, ...items)),
    me === undefined || !mayVouch ? null : addMemberForm(held, me.role, (added) => void drawMembers(main, held, null, added)),
    dialog,
  );
  title.focus();
}
