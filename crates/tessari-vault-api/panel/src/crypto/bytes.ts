// Byte helpers for the encodings every sealed value and every signed statement is built from. Integers are big-endian;
// a variable-length field is always preceded by its length as four bytes, so no field can move another's boundary.

import { VaultCryptoError } from "./errors.ts";

export type Bytes = Uint8Array<ArrayBuffer>;

const ENCODER = new TextEncoder();

export function utf8(text: string): Bytes {
  return ENCODER.encode(text);
}

export function concat(...parts: readonly Uint8Array[]): Bytes {
  const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

/** `value` as `width` big-endian bytes; refuses a value that does not fit. */
export function integer(value: number, width: 1 | 2 | 4 | 8): Bytes {
  if (!Number.isSafeInteger(value) || value < 0 || (width < 8 && value >= 2 ** (8 * width))) {
    throw new VaultCryptoError("malformed");
  }
  const out = new Uint8Array(width);
  let rest = BigInt(value);
  for (let index = width - 1; index >= 0; index -= 1) {
    out[index] = Number(rest & 0xffn);
    rest >>= 8n;
  }
  return out;
}

/** A length-prefixed field: the length as four big-endian bytes, then the bytes. */
export function field(bytes: Uint8Array): Bytes {
  return concat(integer(bytes.length, 4), bytes);
}

export function random(length: number): Bytes {
  return crypto.getRandomValues(new Uint8Array(length));
}

/** Equality that does not stop at the first difference; for values an attacker may be probing. */
export function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) {
    return false;
  }
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) {
    difference |= (left[index] ?? 0) ^ (right[index] ?? 0);
  }
  return difference === 0;
}

/** Overwrite secret bytes the module no longer needs. JavaScript gives no stronger guarantee than this. */
export function wipe(...buffers: Uint8Array[]): void {
  for (const buffer of buffers) {
    buffer.fill(0);
  }
}

/** Reads fields off a byte string in order; any read past the end, or bytes left over, is `malformed`. */
export class Reader {
  readonly #bytes: Bytes;
  #offset = 0;

  constructor(bytes: Bytes) {
    this.#bytes = bytes;
  }

  take(length: number): Bytes {
    if (length > this.#bytes.length - this.#offset) {
      throw new VaultCryptoError("malformed");
    }
    const out = this.#bytes.slice(this.#offset, this.#offset + length);
    this.#offset += length;
    return out;
  }

  integer(width: 1 | 2 | 4 | 8): number {
    let value = 0n;
    for (const byte of this.take(width)) {
      value = (value << 8n) | BigInt(byte);
    }
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new VaultCryptoError("malformed");
    }
    return Number(value);
  }

  field(): Bytes {
    return this.take(this.integer(4));
  }

  rest(): Bytes {
    return this.take(this.#bytes.length - this.#offset);
  }

  end(): void {
    if (this.#offset !== this.#bytes.length) {
      throw new VaultCryptoError("malformed");
    }
  }
}

/** Lower-case hex, for key ids used as map keys; never for secret material. */
export function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
