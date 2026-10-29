// The members page: everyone in the company with their key's state and fingerprint, the person's own fingerprint to
// read out to an admin, and for owners and admins a way to vouch for a key that is waiting and to add someone. Vouching is a statement the
// browser signs and checks against the whole chain before the server keeps it. The same people keep the chain in step
// with the company: admin standing follows each role, and a key still vouched for after its holder left is ended.

import { call, ignored } from "../api.ts";
import { readChain } from "../chain-model.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { verifyChain, type Statement } from "../crypto/statements.ts";
import { el, fill } from "../dom.ts";
import { heading, refusal } from "../screen.ts";
import type { Unlocked } from "../vault.ts";
import { CHAIN_REFUSED, loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import { readDirectory } from "../records/shares-model.ts";
import { memberRows, statementBody, type MemberRow } from "./members-model.ts";
import { departedAccounts, leaveStatement, standingOf, standingStatements, vouchStatements } from "./standing.ts";
import { keyStateWords, vouchFailure, vouchRefused } from "./members-words.ts";
import { addMemberForm, invitedBlock, type Invited } from "./add-member.ts";
import { mayManage } from "./member-admin.ts";

const ROLE_WORDS = { owner: "Owner", admin: "Admin", member: "Member" } as const;

/** Draws the members page into `main`; `notice` is said at its top, as after a vouch the chain had moved past, and
 *  `invited` shows the join link of someone just added. */
export async function drawMembers(main: HTMLElement, held: Unlocked, pageOf: (account: string) => string, notice: string | null = null, invited: Invited | null = null): Promise<void> {
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
  const again = (words: string | null = null): void => void drawMembers(main, held, pageOf, words);
  const signer = { privateKey: held.keys.signingPrivate, publicBytes: held.keys.signingPublic };
  /** Sends `made` in order after the chain this page read; null once all are kept, else what to say. */
  const send = async (made: readonly Statement[]): Promise<{ words: string; stale: boolean } | null> => {
    for (const [index, statement] of made.entries()) {
      const answer = await call("POST", `${path}/statements`, ignored, { body: statementBody(chain.value.statements.length + 1 + index, statement) });
      if (!answer.ok) {
        return { words: vouchFailure(answer.status, answer.code, loadFailure(answer)), stale: answer.code === "stale_chain" };
      }
    }
    return null;
  };
  /** Makes statements with `make` and sends them, redrawing the page once kept or saying why not in `say`. */
  const act = (make: () => Promise<readonly Statement[]>, say: (words: string) => void): void => {
    void make()
      .then(send)
      .then((failed) => (failed === null || failed.stale ? again(failed?.words ?? null) : say(failed.words)))
      .catch((problem: unknown) => say(vouchRefused(problem instanceof VaultCryptoError ? problem.reason : "chain-broken")));
  };

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
    act(
      () => vouchStatements(held.company, owner, chain.value.statements, signer, row.account, row.role, key),
      (words) => {
        confirm.disabled = false;
        fill(dialogSaid, refusal(words));
      },
    );
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
    const standing = row.state === "vouched" ? standingOf(row.role, row.account, vouched, owner) : "matches";
    const vouchedKey = directory.value.get(row.account);
    let align: HTMLButtonElement | null = null;
    if (mayVouch && !row.suspended && standing !== "matches" && vouchedKey !== undefined) {
      const give = standing === "needs-admin";
      const button = el("button", { type: "button", "aria-label": `${give ? "Give" : "End"} ${row.label}'s admin standing` }, give ? "Give admin standing" : "End admin standing");
      button.addEventListener("click", () => {
        button.disabled = true;
        act(() => standingStatements(held.company, owner, chain.value.statements, signer, row.account, vouchedKey, standing), again);
      });
      align = button;
    }
    const shownName = row.account === held.account ? `${row.label} (you)` : row.label;
    return el(
      "li",
      { class: "share member" },
      el("div", {}, me !== undefined && mayManage(me.role, row.role) ? el("a", { class: "share-name", href: pageOf(row.account) }, shownName) : el("span", { class: "share-name" }, shownName), el("span", { class: "detail" }, row.detail)),
      el("span", { class: "muted" }, ROLE_WORDS[row.role]),
      row.suspended ? el("span", { class: "state" }, "Suspended") : el("span", { class: `state ${row.state}` }, keyStateWords(row.state)),
      vouch ?? align ?? el("span", {}),
    );
  });
  const departed = mayVouch ? departedAccounts(members.value, vouched, owner) : [];
  const left = departed.map((account) => {
    const end = el("button", { type: "button", "aria-label": `End the key of former member ${account.slice(0, 8)}` }, "End their key");
    end.addEventListener("click", () => {
      end.disabled = true;
      act(() => leaveStatement(held.company, owner, chain.value.statements, signer, account), again);
    });
    return el("li", { class: "share member" }, el("div", {}, el("span", { class: "share-name" }, "A former member"), el("span", { class: "detail" }, `Account ${account.slice(0, 8)}`)), end);
  });
  const own = me?.fingerprint ?? null;
  fill(
    main,
    el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "Records are shared only with keys an owner or admin has vouched for, after comparing the key's fingerprint with its holder."))),
    notice === null ? null : refusal(notice),
    invited === null ? null : invitedBlock(invited),
    own === null ? null : el("section", { class: "record-section", "aria-labelledby": "own-key" }, el("h2", { id: "own-key" }, "Your key's fingerprint"), el("p", { class: "fingerprint" }, own)),
    el("section", { class: "record-section", "aria-labelledby": "everyone" }, el("h2", { id: "everyone" }, "Everyone in the company"), el("ul", { class: "shares" }, ...items)),
    left.length === 0
      ? null
      : el(
          "section",
          { class: "record-section", "aria-labelledby": "left" },
          el("h2", { id: "left" }, "Keys of people who left"),
          el("p", { class: "muted" }, "These people are no longer in the company, but their keys still count. End each key: what they wrote stays readable, and nothing new can be written in their name."),
          el("ul", { class: "shares" }, ...left),
        ),
    me === undefined || !mayVouch ? null : addMemberForm(held, me.role, (added) => void drawMembers(main, held, pageOf, null, added)),
    dialog,
  );
  title.focus();
}
