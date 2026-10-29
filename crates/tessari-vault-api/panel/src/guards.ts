// The router's guards, and where a person goes once a guard has had them sign in or unlock. Kept apart from the views
// so they are tested.

import type { Verdict } from "./router/router.ts";

const SIGN_IN = "/sign-in";
const UNLOCK = "/unlock";

/** A guard sending the person to `gate` unless `open()`, carrying where they were going as `next`. */
function gated(gate: string, open: () => boolean, current: () => string): () => Verdict {
  return () => (open() ? { kind: "allow" } : { kind: "redirect", to: `#${gate}?next=${encodeURIComponent(current().replace(/^#/, ""))}` });
}

/** A guard sending a person with no session to sign in. */
export const needsSession = (signedIn: () => boolean, current: () => string): (() => Verdict) => gated(SIGN_IN, signedIn, current);

/** A guard sending a person whose vault is locked to unlock it. */
export const needsUnlock = (unlocked: () => boolean, current: () => string): (() => Verdict) => gated(UNLOCK, unlocked, current);

/** Guards checked in order; the first that does not allow decides. */
export function every(...guards: readonly (() => Verdict)[]): () => Verdict {
  return () => {
    for (const guard of guards) {
      const verdict = guard();
      if (verdict.kind !== "allow") {
        return verdict;
      }
    }
    return { kind: "allow" };
  };
}

/** `next` when it is a path of this page other than the `gates`, else the records list. */
function inPage(next: string | null, gates: readonly string[]): string {
  if (next === null || !next.startsWith("/") || next.startsWith("//") || gates.some((gate) => next.startsWith(gate))) {
    return "#/";
  }
  return `#${next}`;
}

/** The page to open after signing in. */
export const afterSignIn = (next: string | null): string => inPage(next, [SIGN_IN]);

/** The page to open after unlocking. */
export const afterUnlock = (next: string | null): string => inPage(next, [SIGN_IN, UNLOCK]);
