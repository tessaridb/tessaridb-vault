// The notices a member has not seen yet, shown above whatever page opens after unlocking: every time an owner read
// their keys kept for recovery or handed them back. Which notices this browser already showed is a convenience kept in
// this browser only; the list itself is always on the recovery page.

import { call } from "../api.ts";
import { el } from "../dom.ts";
import { companyPath } from "../records/load.ts";
import { readMemberList } from "../records/share-targets.ts";
import type { Unlocked } from "../vault.ts";
import { newNotices, noticeWords, readNotices } from "./recovery-model.ts";

let banner: HTMLElement | null = null;

const seenKey = (held: Unlocked): string => `vault-notices-seen:${held.account}:${held.company}`;

function seenAt(held: Unlocked): number | null {
  try {
    const kept = Number(localStorage.getItem(seenKey(held)));
    return Number.isSafeInteger(kept) && kept > 0 ? kept : null;
  } catch {
    return null;
  }
}

function markSeen(held: Unlocked, at: number): void {
  try {
    localStorage.setItem(seenKey(held), String(at));
  } catch {
    // Without storage the notices show again next time, which is the safe way round.
  }
}

/** Puts the banner, while there is one, at the top of `view`, above the pane the page draws into. */
export function placeBanner(view: HTMLElement): void {
  if (banner !== null) {
    view.prepend(banner);
  }
}

/** Drops the banner, as when the vault locks. */
export function dropBanner(): void {
  banner?.remove();
  banner = null;
}

/** Reads the notices about the unlocked member and shows the ones this browser has not shown, at the top of `view`. */
export async function checkNotices(held: Unlocked, view: () => HTMLElement, recoveryHref: string): Promise<void> {
  const path = companyPath(held);
  const [notices, members] = await Promise.all([call("GET", `${path}/notices/me`, readNotices), call("GET", `${path}/members`, readMemberList)]);
  if (!notices.ok) {
    return;
  }
  const fresh = newNotices(notices.value, seenAt(held));
  const last = fresh.at(-1);
  if (last === undefined) {
    return;
  }
  const names = new Map((members.ok ? members.value : []).map((member) => [member.account, member.name]));
  const when = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  const gotIt = el("button", { type: "button" }, "Got it");
  banner = el(
    "section",
    { class: "notice-banner", "aria-labelledby": "notices-title" },
    el("h2", { id: "notices-title" }, "Since you last looked"),
    el("ul", {}, ...fresh.map((notice) => el("li", {}, `${noticeWords(notice.kind, names.get(notice.by) ?? "An owner")}, ${when.format(new Date(notice.at * 1000))}.`))),
    el("p", {}, el("a", { href: recoveryHref }, "See recovery"), " ", gotIt),
  );
  gotIt.addEventListener("click", () => {
    markSeen(held, last.at);
    dropBanner();
  });
  placeBanner(view());
}
