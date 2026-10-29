// Adding a passkey besides the page: the options the server gives a registration, the request the browser makes from
// them, and the words for each refusal. No DOM here, so all of it is tested.

import type { Failure } from "../api.ts";
import { decode } from "../base64url.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** What `POST /api/me/passkeys/begin` answers, decoded. */
export interface RegistrationOptions {
  readonly challenge: Uint8Array<ArrayBuffer>;
  readonly rpId: string;
  readonly userId: Uint8Array<ArrayBuffer>;
  readonly userName: string;
  /** The person's passkeys already registered, so the browser does not make a second on the same authenticator. */
  readonly exclude: readonly Uint8Array<ArrayBuffer>[];
}

export function readRegistrationOptions(v: unknown): RegistrationOptions | null {
  if (!isRecord(v) || typeof v["challenge"] !== "string" || typeof v["rp_id"] !== "string" || typeof v["user_id"] !== "string" || typeof v["user_name"] !== "string" || !Array.isArray(v["exclude"])) {
    return null;
  }
  const challenge = decode(v["challenge"]);
  const userId = decode(v["user_id"]);
  const exclude = v["exclude"].map((each: unknown) => (typeof each === "string" ? decode(each) : null));
  if (challenge === null || userId === null || !exclude.every((each) => each !== null)) {
    return null;
  }
  return { challenge, rpId: v["rp_id"], userId, userName: v["user_name"], exclude };
}

/** The algorithms the vault verifies, as COSE numbers: EdDSA, ES256, RS256. */
const ALGORITHMS = [-8, -7, -257] as const;

/** The browser's request for a new passkey: discoverable, so it signs in without naming an account, and verified. */
export function creationOptions(options: RegistrationOptions, displayName: string): PublicKeyCredentialCreationOptions {
  return {
    challenge: options.challenge,
    rp: { id: options.rpId, name: "TessariDB Vault" },
    user: { id: options.userId, name: options.userName, displayName },
    pubKeyCredParams: ALGORITHMS.map((alg) => ({ type: "public-key", alg })),
    authenticatorSelection: { residentKey: "required", userVerification: "required" },
    excludeCredentials: options.exclude.map((id) => ({ type: "public-key", id })),
    attestation: "none",
  };
}

/** What went wrong and what to do, for a passkey that wasn't added. */
export function passkeyFailure(failure: Failure): string {
  switch (failure.code) {
    case "passkey_registered":
      return "That passkey is already registered with your account.";
    case "cancelled":
      return "The passkey prompt was cancelled, so nothing was added.";
    case "passkey_refused":
      return "That passkey wasn't accepted here. Try again, or use another device.";
    case "network":
      return "The vault didn't answer. Check your connection, then try again.";
    default:
      return "The vault couldn't add the passkey. Try again; if it keeps failing, tell your admin.";
  }
}
