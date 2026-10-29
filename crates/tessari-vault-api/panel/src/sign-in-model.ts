// What signing in needs besides the page: the answers it reads, what each refusal means in words, and how an
// identity provider's return is recognised. No DOM here, so all of it is tested.

import type { Failure } from "./api.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** The ways in this server offers. */
export interface Methods {
  readonly password: boolean;
  readonly passkey: boolean;
  readonly provider: boolean;
}

export function readMethods(v: unknown): Methods | null {
  if (!isRecord(v)) {
    return null;
  }
  const { password, passkey, provider } = v;
  return typeof password === "boolean" && typeof passkey === "boolean" && typeof provider === "boolean"
    ? { password, passkey, provider }
    : null;
}

/** The step between the password and the code. */
export function readChallenge(v: unknown): string | null {
  return isRecord(v) && typeof v["challenge"] === "string" && v["challenge"] !== "" ? v["challenge"] : null;
}

/** A session token, as every way in answers it. */
export function readSession(v: unknown): string | null {
  return isRecord(v) && typeof v["session"] === "string" && v["session"] !== "" ? v["session"] : null;
}

/** What an identity provider's return completed: a sign-in, with its session, or a link of the identity. */
export type Returned = { readonly kind: "session"; readonly session: string } | { readonly kind: "linked" };

/** The answer of `GET /api/sign-in/oidc/return`: a session, or no body once a link is made. */
export function readReturned(v: unknown): Returned | null {
  if (v === null) {
    return { kind: "linked" };
  }
  const session = readSession(v);
  return session === null ? null : { kind: "session", session };
}

/** The issuer address `POST /api/me/identities/oidc` answers, which the page then opens: http or https only. */
export function readLinkAddress(v: unknown): string | null {
  if (!isRecord(v) || typeof v["address"] !== "string" || !URL.canParse(v["address"])) {
    return null;
  }
  const { protocol } = new URL(v["address"]);
  return protocol === "https:" || protocol === "http:" ? v["address"] : null;
}

/** What the browser needs to ask for a passkey. */
export interface PasskeyRequest {
  readonly challenge: string;
  readonly rpId: string;
}

export function readPasskeyRequest(v: unknown): PasskeyRequest | null {
  return isRecord(v) && typeof v["challenge"] === "string" && typeof v["rp_id"] === "string"
    ? { challenge: v["challenge"], rpId: v["rp_id"] }
    : null;
}

/** What went wrong and what to do, for a refused sign-in step. */
export function signInFailure(failure: Failure): string {
  switch (failure.code) {
    case "sign_in_failed":
      return "That email and password don't open an account here. Check both and try again.";
    case "wrong_code":
      return "That code didn't match. Use the newest code from your authenticator app.";
    case "passkey_refused":
      return "That passkey wasn't accepted here. Try again, or sign in with your password.";
    case "throttled":
      return "Too many tries from here. Wait a few minutes, then try again.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    case "unavailable":
      return "The vault can't sign you in right now. Try again in a minute.";
    default:
      return failure.status >= 500
        ? "The vault couldn't finish signing you in. Try again; if it keeps failing, tell your admin."
        : "The vault refused this sign-in. Try again.";
  }
}

/** Whether `search` (a location's query string) is an identity provider sending the person back. */
export const isProviderReturn = (search: string): boolean => {
  const query = new URLSearchParams(search);
  return query.has("state") && (query.has("code") || query.has("error"));
};

/** Where the provider sign-in starts: the server answers with the redirect. */
export const PROVIDER_START = "/api/sign-in/oidc";
