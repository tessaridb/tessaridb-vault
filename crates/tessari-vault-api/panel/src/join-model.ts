// What joining with an enrolment link needs besides the page: the authenticator setup it reads, the setup key as the
// person types it into an app, the sign-in password rule the server applies, and the words each refusal becomes. No
// DOM here, so all of it is tested.

import type { Failure } from "./api.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** The second factor the server made for the person joining: its key and the link authenticator apps open. */
export interface Authenticator {
  readonly secret: string;
  readonly uri: string;
}

/** Only a time-based otpauth link is kept: the page draws it as a link, so no other scheme may reach an href. */
export function readAuthenticator(v: unknown): Authenticator | null {
  if (!isRecord(v)) {
    return null;
  }
  const { secret, uri } = v;
  return typeof secret === "string" && secret !== "" && typeof uri === "string" && uri.startsWith("otpauth://totp/")
    ? { secret, uri }
    : null;
}

/** `secret` in groups of four, the way a person reads it into an authenticator app. */
export const groupedKey = (secret: string): string => (secret.match(/.{1,4}/g) ?? []).join(" ");

/** The enrolment token in what a person pasted: the code itself, or a whole link carrying it. */
export function tokenFrom(typed: string): string {
  const text = typed.trim();
  const at = text.indexOf("token=");
  if (at < 0) {
    return text;
  }
  const carried = text.slice(at + "token=".length).split("&")[0] ?? "";
  try {
    return decodeURIComponent(carried);
  } catch {
    return carried;
  }
}

/** Why `password` (typed again as `again`) cannot be the new sign-in password, or null when it can. */
export function passwordProblem(password: string, again: string): string | null {
  // Counted as the server counts: characters of the NFC form.
  const length = [...password.normalize("NFC")].length;
  if (length < 8 || length > 256) {
    return "A sign-in password is 8 to 256 characters.";
  }
  return password === again ? null : "The two passwords don't match. Type the same one twice.";
}

/** What went wrong and what to do, for a refused join step. */
export function joinFailure(failure: Failure): string {
  switch (failure.code) {
    case "not_found":
      return "This link was used already or has expired. Ask your admin for a new one.";
    case "wrong_code":
      return "That code didn't match. Use the newest code from your authenticator app.";
    case "weak_password":
      return "That password can't be used: 8 to 256 characters, without your email address or the part before the @.";
    case "throttled":
      return "Too many tries from here. Wait a few minutes, then try again.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    default:
      return "The vault couldn't finish setting up your account. Try again; if it keeps failing, tell your admin.";
  }
}
