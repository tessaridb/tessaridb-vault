// One-time password fields: the seed is stored in the record, sealed like every other field, and the live code is
// computed in the browser (RFC 6238 over RFC 4226 truncation). SHA-1 is supported because authenticators issue it;
// HMAC-SHA-1 remains sound as a MAC even though SHA-1 is not as a hash.

import { integer, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";

export type OtpAlgorithm = "SHA1" | "SHA256" | "SHA512";

export interface OtpParams {
  secret: Bytes;
  algorithm: OtpAlgorithm;
  digits: 6 | 8;
  /** Seconds per step; 30 unless the issuer said otherwise. */
  period: number;
}

const HASH: Record<OtpAlgorithm, string> = { SHA1: "SHA-1", SHA256: "SHA-256", SHA512: "SHA-512" };

/** The code for `unixSeconds`, as the authenticator shows it (left-padded with zeros). */
export async function totpCode(params: OtpParams, unixSeconds: number): Promise<string> {
  if (!Number.isSafeInteger(params.period) || params.period < 1 || !Number.isSafeInteger(unixSeconds) || unixSeconds < 0) {
    throw new VaultCryptoError("malformed");
  }
  const key = await crypto.subtle.importKey("raw", params.secret, { name: "HMAC", hash: HASH[params.algorithm] }, false, [
    "sign",
  ]);
  const step = integer(Math.floor(unixSeconds / params.period), 8);
  const mac = new Uint8Array(await crypto.subtle.sign("HMAC", key, step));
  const offset = (mac[mac.length - 1] ?? 0) & 0x0f;
  const binary =
    (((mac[offset] ?? 0) & 0x7f) << 24) |
    ((mac[offset + 1] ?? 0) << 16) |
    ((mac[offset + 2] ?? 0) << 8) |
    (mac[offset + 3] ?? 0);
  return String(binary % 10 ** params.digits).padStart(params.digits, "0");
}

const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/** Base32 (RFC 4648 §6) with padding, as issuers print seeds. */
export function base32Encode(bytes: Uint8Array): string {
  let out = "";
  let buffer = 0;
  let bits = 0;
  for (const byte of bytes) {
    buffer = (buffer << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += ALPHABET[(buffer >> (bits - 5)) & 0x1f] ?? "";
      bits -= 5;
    }
    buffer &= (1 << bits) - 1;
  }
  if (bits > 0) {
    out += ALPHABET[(buffer << (5 - bits)) & 0x1f] ?? "";
  }
  return out.padEnd(Math.ceil(out.length / 8) * 8, "=");
}

/** Decode a pasted seed: case and spaces ignored, padding optional; leftover bits must be zero. */
export function base32Decode(text: string): Bytes {
  const clean = text.replace(/\s+/g, "").toUpperCase().replace(/=+$/, "");
  const out: number[] = [];
  let buffer = 0;
  let bits = 0;
  for (const character of clean) {
    const value = ALPHABET.indexOf(character);
    if (value < 0) {
      throw new VaultCryptoError("malformed");
    }
    buffer = (buffer << 5) | value;
    bits += 5;
    if (bits >= 8) {
      out.push((buffer >> (bits - 8)) & 0xff);
      bits -= 8;
    }
    buffer &= (1 << bits) - 1;
  }
  if (bits >= 5 || buffer !== 0) {
    throw new VaultCryptoError("malformed");
  }
  return Uint8Array.from(out);
}
