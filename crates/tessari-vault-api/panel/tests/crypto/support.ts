// Helpers the crypto tests share: hex in and out, and an assertion that a call is refused for one named reason.

import assert from "node:assert/strict";

import { VaultCryptoError, type Refusal } from "../../src/crypto/errors.ts";

export function fromHex(text: string): Uint8Array<ArrayBuffer> {
  const clean = text.replace(/\s+/g, "");
  assert.equal(clean.length % 2, 0, "hex has an even length");
  const out = new Uint8Array(clean.length / 2);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = Number.parseInt(clean.slice(index * 2, index * 2 + 2), 16);
  }
  return out;
}

export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** Assert that `action` is refused with exactly `reason`; any other failure, or success, fails the test. */
export async function refusedWith(action: Promise<unknown>, reason: Refusal): Promise<void> {
  await assert.rejects(action, (error: unknown) => {
    assert.ok(error instanceof VaultCryptoError, `a VaultCryptoError, got ${String(error)}`);
    assert.equal(error.reason, reason);
    return true;
  });
}

/** A copy of `bytes` with one bit flipped at `index`. */
export function flipped(bytes: Uint8Array<ArrayBuffer>, index: number): Uint8Array<ArrayBuffer> {
  const copy = bytes.slice();
  copy[index] = (copy[index] ?? 0) ^ 0x01;
  return copy;
}

/** `size` random bytes for test data; `getRandomValues` fills at most 65 536 bytes a call. */
export function noise(size: number): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(size);
  for (let at = 0; at < size; at += 65_536) {
    crypto.getRandomValues(out.subarray(at, Math.min(size, at + 65_536)));
  }
  return out;
}
