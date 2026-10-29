// Reading what a person pastes into a one-time password field: the setup key an issuer shows, or the otpauth:// link
// its QR code carries (the key URI format authenticator apps share). Only time-based codes are kept; no DOM here, so
// it is tested.

import type { OtpValue } from "./fields.ts";
import { base32Decode, type OtpAlgorithm } from "./totp.ts";

const ALGORITHMS: readonly OtpAlgorithm[] = ["SHA1", "SHA256", "SHA512"];

/** `text` as a base32 secret in upper case without spaces or padding, or null when it isn't one. */
function secretOf(text: string): string | null {
  const clean = text.replace(/\s+/g, "").toUpperCase().replace(/=+$/, "");
  try {
    return base32Decode(clean).length === 0 ? null : clean;
  } catch {
    return null;
  }
}

/** The field value `text` describes: a setup key, or an otpauth:// link for a time-based code; null when it is neither. */
export function otpFromInput(text: string): OtpValue | null {
  const trimmed = text.trim();
  if (!/^otpauth:/i.test(trimmed)) {
    const secret = secretOf(trimmed);
    return secret === null ? null : { secret, algorithm: "SHA1", digits: 6, period: 30 };
  }
  let link: URL;
  try {
    link = new URL(trimmed);
  } catch {
    return null;
  }
  if (link.host.toLowerCase() !== "totp") {
    return null;
  }
  const secret = secretOf(link.searchParams.get("secret") ?? "");
  const algorithm = ALGORITHMS.find((each) => each === (link.searchParams.get("algorithm") ?? "SHA1").toUpperCase());
  const digits = Number(link.searchParams.get("digits") ?? "6");
  const period = Number(link.searchParams.get("period") ?? "30");
  if (secret === null || algorithm === undefined || (digits !== 6 && digits !== 8) || !Number.isInteger(period) || period < 1 || period > 3600) {
    return null;
  }
  return { secret, algorithm, digits, period };
}
