// The recovery page. Every member sees whether their keys are kept for recovery and every time an owner reached them.
// An owner also sees the company recovery key (made here once, by the browser holding the company owner key) and the
// open requests, each handed over only after the member reads out the code their own screen shows.

import { call, type Failure } from "../api.ts";
import { readChain, type Chain } from "../chain-model.ts";
import { sameBytes } from "../crypto/bytes.ts";
import { handoffCode } from "../crypto/fingerprint.ts";
import { verifyChain, type Vouched } from "../crypto/statements.ts";
import { el, fill } from "../dom.ts";
import { CHAIN_REFUSED, loadFailure } from "../records/list-view.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList, type ListedMember } from "../records/share-targets.ts";
import { readDirectory, type DirectoryKey } from "../records/shares-model.ts";
import { heading, refusal } from "../screen.ts";
import { keyIdBytes } from "../unlock-model.ts";
import type { Unlocked } from "../vault.ts";
import { handOverRequest, makeRecoveryKey } from "./hand-over.ts";
import { noticeWords, readNotices, readOwnEscrow, readRequests, recoveryFailure, type Notice, type OpenRequest } from "./recovery-model.ts";

const when = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
const at = (seconds: number): string => when.format(new Date(seconds * 1000));

const section = (id: string, title: string, ...body: Array<HTMLElement | null>): HTMLElement =>
  el("section", { class: "record-section", "aria-labelledby": id }, el("h2", { id }, title), ...body);

/** The caller's own section: whether their keys are kept for recovery, and who reached them when. */
function yourKeys(escrowed: boolean, recoveryKeyed: boolean, notices: readonly Notice[], names: ReadonlyMap<string, string>): HTMLElement {
  const status = escrowed
    ? "Your keys are kept for recovery. If you forget your unlock password, an owner can hand them back to you."
    : recoveryKeyed
      ? "Your keys aren't kept for recovery yet. They will be the next time you unlock."
      : "Your keys aren't kept for recovery: the company has no recovery key yet.";
  const reached =
    notices.length === 0
      ? el("p", { class: "muted" }, "No owner has reached your keys.")
      : el("ul", { class: "shares" }, ...[...notices].reverse().map((notice) => el("li", { class: "share" }, el("span", {}, noticeWords(notice.kind, names.get(notice.by) ?? "An owner"), el("span", { class: "detail" }, at(notice.at))))));
  return section("your-keys", "Your keys", el("p", { class: "muted" }, status), el("h3", {}, "Who reached them"), reached);
}

/** The hand-over dialog for one request: the code to compare, then the act. */
function handOverDialog(held: Unlocked, vouched: Vouched, request: OpenRequest, name: string, key: DirectoryKey, done: () => void): { readonly dialog: HTMLDialogElement; readonly open: () => void } {
  const code = el("p", { class: "fingerprint" });
  const said = el("div", { "aria-live": "polite" });
  const notNow = el("button", { type: "button", autofocus: "" }, "Not now");
  const confirm = el("button", { type: "button", class: "primary" }, "The codes match: hand over");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": "hand-over-title" },
    el("h2", { id: "hand-over-title" }, `Hand ${name}'s keys back?`),
    el("p", {}, `Ask ${name} to read you the code on their screen. Hand over only when it is this one:`),
    code,
    el("p", { class: "muted" }, `${name} then sets a new unlock password, and sees that you did this. It is on the company's trail too.`),
    said,
    el("div", { class: "actions" }, notNow, confirm),
  );
  notNow.addEventListener("click", () => dialog.close());
  confirm.addEventListener("click", () => {
    confirm.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Handing the keys over."));
    void handOverRequest(held, vouched, request, key).then((failed) => {
      confirm.disabled = false;
      if (failed === null) {
        dialog.close();
        done();
        return;
      }
      fill(said, refusal(recoveryFailure(failed)));
    });
  });
  const open = (): void => {
    fill(said, null);
    void handoffCode(request.handoffPublic).then((words) => {
      code.textContent = words;
      dialog.showModal();
    });
  };
  return { dialog, open };
}

/** The open requests, each with its hand-over. */
function requestsList(held: Unlocked, vouched: Vouched, requests: readonly OpenRequest[], members: ReadonlyMap<string, ListedMember>, directory: ReadonlyMap<string, DirectoryKey>, again: (words: string) => void): HTMLElement {
  if (requests.length === 0) {
    return el("p", { class: "muted" }, "Nobody has asked for recovery.");
  }
  const rows = requests.map((request) => {
    const name = members.get(request.account)?.name ?? "A former member";
    const key = directory.get(request.account);
    const who = el("span", { class: "share-name" }, name, el("span", { class: "detail" }, `Asked ${at(request.created)}`));
    if (key === undefined) {
      return el("li", { class: "share" }, who, el("span", { class: "muted" }, "No key set"));
    }
    const { dialog, open } = handOverDialog(held, vouched, request, name, key, () => again(`${name}'s keys are handed over. They can now set a new unlock password.`));
    const button = el("button", { type: "button" }, "Hand over");
    button.setAttribute("aria-label", `Hand over ${name}'s keys`);
    button.addEventListener("click", open);
    return el("li", { class: "share" }, who, button, dialog);
  });
  return el("ul", { class: "shares" }, ...rows);
}

/** The owner's section: the recovery key, made here when this browser holds the owner key, and the open requests. */
async function ownersPart(held: Unlocked, chain: Chain, vouched: Vouched, members: readonly ListedMember[], directory: ReadonlyMap<string, DirectoryKey>, again: (words: string) => void): Promise<HTMLElement> {
  const byAccount = new Map(members.map((member) => [member.account, member]));
  if (vouched.recoveryKey() === undefined) {
    if (!sameBytes(held.keys.signingPublic, held.keys.ownerSigningPublic)) {
      return section("recovery-key", "Company recovery key", el("p", { class: "muted" }, "The company has no recovery key yet. Only the owner who opened the company's vault first can make it, because the company owner key vouches for it."));
    }
    const said = el("div", { "aria-live": "polite" });
    const make = el("button", { type: "button", class: "primary" }, "Make the recovery key");
    make.addEventListener("click", () => {
      make.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Making the recovery key."));
      const owners = members.flatMap((member) => {
        const key = member.role === "owner" && member.active ? directory.get(member.account) : undefined;
        const keyId = key === undefined ? null : keyIdBytes(key.keyId);
        return key === undefined || keyId === null ? [] : [{ account: member.account, keyId, agreementPublic: key.agreementPublic }];
      });
      void makeRecoveryKey(held, chain, vouched, owners).then((failed: Failure | null) => {
        make.disabled = false;
        if (failed === null) {
          again("The recovery key is made. Each member's keys are kept for recovery the next time they unlock, yours included.");
          return;
        }
        fill(said, refusal(recoveryFailure(failed)));
      });
    });
    const lead = "With it, the company's owners can hand a member their keys back when the member forgets their unlock password. Every member's keys are wrapped to it, so owners can reach them, and each member sees every time that happens.";
    return section("recovery-key", "Company recovery key", el("p", { class: "muted" }, lead), make, said);
  }
  const requests = await call("GET", `${companyPath(held)}/recovery-requests`, readRequests);
  const listed = requests.ok ? requestsList(held, vouched, requests.value, byAccount, directory, again) : refusal(loadFailure(requests));
  return section("recovery-key", "Recovery requests", el("p", { class: "muted" }, "The company has a recovery key. A request stays open for 3 days."), listed);
}

/** Draws the recovery page into `main`; `notice` is said at its top, as after a hand-over. */
export async function drawRecovery(main: HTMLElement, held: Unlocked, notice: string | null = null): Promise<void> {
  const title = heading("Recovery");
  const head = el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "When someone forgets their unlock password, an owner can hand their keys back. Nobody else can, the vault's operator included.")));
  const said = el("div", { "aria-live": "polite" }, notice === null ? el("p", { class: "muted", role: "status" }, "Loading.") : el("p", { class: "muted", role: "status" }, notice));
  fill(main, head, said);
  title.focus();
  const path = companyPath(held);
  const [chain, own, notices, members, directory] = await Promise.all([
    call("GET", `${path}/statements`, readChain),
    call("GET", `${path}/keys/me/escrow`, readOwnEscrow),
    call("GET", `${path}/notices/me`, readNotices),
    call("GET", `${path}/members`, readMemberList),
    call("GET", `${path}/keys`, readDirectory),
  ]);
  const failed = [chain, notices, members, directory].find((answer) => !answer.ok);
  if (failed !== undefined && !failed.ok) {
    fill(said, refusal(loadFailure(failed)));
    return;
  }
  if (!chain.ok || !notices.ok || !members.ok || !directory.ok || (!own.ok && own.status !== 404)) {
    fill(said, refusal("The vault couldn't load recovery. Try again in a minute."));
    return;
  }
  const vouched = await verifyChain(held.company, held.keys.ownerSigningPublic, chain.value.statements).catch(() => null);
  if (vouched === null) {
    fill(said, refusal(CHAIN_REFUSED));
    return;
  }
  const names = new Map(members.value.map((member) => [member.account, member.name]));
  const standing = vouched.recoveryKey();
  const parts = [yourKeys(own.ok && own.value === standing?.keyId, standing !== undefined, notices.value, names)];
  const again = (words: string): void => void drawRecovery(main, held, words);
  if (members.value.find((member) => member.account === held.account)?.role === "owner") {
    parts.unshift(await ownersPart(held, chain.value, vouched, members.value, directory.value, again));
  }
  fill(main, head, notice === null ? null : said, ...parts);
}
