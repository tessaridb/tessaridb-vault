// The router's guards, and where a person goes after signing in. Kept apart from the views so they are tested.

import type { Verdict } from "./router/router.ts";

const SIGN_IN = "/sign-in";

/** A guard sending a person with no session to sign in, carrying where they were going as `next`. */
export function needsSession(signedIn: () => boolean, current: () => string): () => Verdict {
  return () =>
    signedIn() ? { kind: "allow" } : { kind: "redirect", to: `#${SIGN_IN}?next=${encodeURIComponent(current().replace(/^#/, ""))}` };
}

/** The page to open after signing in: `next` when it is a path of this page, else the records list. */
export function afterSignIn(next: string | null): string {
  if (next === null || !next.startsWith("/") || next.startsWith("//") || next.startsWith(SIGN_IN)) {
    return "#/";
  }
  return `#${next}`;
}
