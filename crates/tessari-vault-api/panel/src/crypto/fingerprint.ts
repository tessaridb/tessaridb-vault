// The fingerprint of a member's key set that two people compare, read aloud or side by side, before one vouches for the
// other's keys: SHA-256 over both public keys under its own label, the first 16 bytes shown as eight groups of four hex
// digits. It is for people to compare and is never used as a key.

import { concat, field, utf8, type Bytes } from "./bytes.ts";
import { LABEL } from "./labels.ts";

const SHOWN_BYTES = 16;

/** The fingerprint of the key set whose public keys are `agreementPublic` and `signingPublic`. */
export async function keyFingerprint(agreementPublic: Bytes, signingPublic: Bytes): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", concat(utf8(LABEL.fingerprint), field(agreementPublic), field(signingPublic))));
  const digits = [...digest.subarray(0, SHOWN_BYTES)].map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join("");
  return digits.match(/.{4}/g)?.join(" ") ?? digits;
}

const CODE_BYTES = 8;

/**
 * The code a member reads out to an owner before the owner hands their keys over: SHA-256 over the member's one-time
 * handoff key under its own label, the first 8 bytes as four groups of four hex digits. A server that put a key of its
 * own in the request would show the owner a code the member does not see.
 */
export async function handoffCode(handoffPublic: Bytes): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", concat(utf8(LABEL.handoffCode), field(handoffPublic))));
  const digits = [...digest.subarray(0, CODE_BYTES)].map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join("");
  return digits.match(/.{4}/g)?.join(" ") ?? digits;
}
