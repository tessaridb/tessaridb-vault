// Unlocking: the unlock password becomes the master key MK in the vault's own WebAssembly module (Argon2id; WebCrypto
// has none), MK becomes the unlock key UK through HKDF, and UK wraps this member's private keys (algorithm 0x03:
// `[version][0x03][key id 16][nonce 12][ciphertext ‖ tag]`). MK exists only between the two steps and is wiped; UK is a
// non-extractable CryptoKey. Every UK wrap uses a random nonce, so UK refuses to make more than 2^20 of them.
//
// Known leak: the password arrives as a JavaScript string, which cannot be overwritten; its UTF-8 copy is wiped.

import { Reader, concat, field, sameBytes, utf8, wipe, random, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { ALGORITHM, FORMAT_VERSION, LABEL } from "./labels.ts";

/** The most wraps one unlock key may make (key-hierarchy worksheet §2, far inside NIST SP 800-38D's 2^32). */
export const MAX_WRAPS = 2 ** 20;

const SALT_BYTES = 16;
const MASTER_KEY_BYTES = 32;
const KEY_ID_BYTES = 16;
const NONCE_BYTES = 12;
const DERIVED = 0;

/** The member's Argon2id parameters, stored with their key bundle and raised over time. */
export interface KdfParams {
  memoryKib: number;
  passes: number;
  lanes: number;
}

/** Which private key a wrap holds, from the member's own key rows. */
export interface MemberKeyContext {
  account: string;
  company: string;
  kind: "agreement" | "signing";
  keyId: Bytes;
}

interface Exports {
  memory: WebAssembly.Memory;
  alloc: (length: number) => number;
  free: (pointer: number, length: number) => void;
  derive: (...args: number[]) => number;
}

function exportsOf(instance: WebAssembly.Instance): Exports {
  const { memory, vault_alloc: alloc, vault_free: free, vault_derive: derive } = instance.exports;
  if (
    !(memory instanceof WebAssembly.Memory) ||
    typeof alloc !== "function" ||
    typeof free !== "function" ||
    typeof derive !== "function"
  ) {
    throw new VaultCryptoError("kdf-refused");
  }
  return {
    memory,
    alloc: (length) => Number(alloc(length)),
    free: (pointer, length) => {
      free(pointer, length);
    },
    derive: (...args) => Number(derive(...args)),
  };
}

/**
 * MK from the unlock password, the member's 16-byte salt and their parameters. The module refuses parameters below
 * its floor; every buffer it lent is cleared and released before this returns.
 */
export async function deriveMasterKey(
  module: WebAssembly.Module,
  password: string,
  salt: Bytes,
  params: KdfParams,
): Promise<Bytes> {
  if (salt.length !== SALT_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  const wasm = exportsOf(await WebAssembly.instantiate(module, {}));
  const passwordBytes = utf8(password);
  const passwordLength = Math.max(passwordBytes.length, 1);
  const passwordAt = wasm.alloc(passwordLength);
  const saltAt = wasm.alloc(SALT_BYTES);
  const outAt = wasm.alloc(MASTER_KEY_BYTES);
  try {
    const memory = new Uint8Array(wasm.memory.buffer);
    memory.set(passwordBytes, passwordAt);
    memory.set(salt, saltAt);
    const status = wasm.derive(
      passwordAt,
      passwordBytes.length,
      saltAt,
      params.memoryKib,
      params.passes,
      params.lanes,
      outAt,
    );
    if (status !== DERIVED) {
      throw new VaultCryptoError("kdf-refused");
    }
    return new Uint8Array(wasm.memory.buffer).slice(outAt, outAt + MASTER_KEY_BYTES);
  } finally {
    wipe(passwordBytes);
    wasm.free(passwordAt, passwordLength);
    wasm.free(saltAt, SALT_BYTES);
    wasm.free(outAt, MASTER_KEY_BYTES);
  }
}

function associatedData(context: MemberKeyContext): Bytes {
  if (context.keyId.length !== KEY_ID_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  return concat(
    utf8(LABEL.memberKey),
    field(utf8(context.account)),
    field(utf8(context.company)),
    field(utf8(context.kind)),
    field(context.keyId),
  );
}

/** Whose key set a pinned owner key belongs to. */
export interface PinContext {
  account: string;
  company: string;
  keyId: Bytes;
}

function pinData(pin: PinContext): Bytes {
  if (pin.keyId.length !== KEY_ID_BYTES) {
    throw new VaultCryptoError("malformed");
  }
  return concat(utf8(LABEL.ownerPin), field(utf8(pin.account)), field(utf8(pin.company)), field(pin.keyId));
}

/** The unlock key, holding the count of wraps made under it so far (stored with the member's bundle). */
export class UnlockKey {
  readonly #key: CryptoKey;
  #wrapsMade: number;

  private constructor(key: CryptoKey, wrapsMade: number) {
    this.#key = key;
    this.#wrapsMade = wrapsMade;
  }

  /** UK = HKDF(MK, "tessaridb-vault/v1/unlock-wrap"); MK is wiped. `wrapsMade` is the stored count. */
  static async fromMasterKey(masterKey: Bytes, wrapsMade: number): Promise<UnlockKey> {
    if (masterKey.length !== MASTER_KEY_BYTES || !Number.isSafeInteger(wrapsMade) || wrapsMade < 0) {
      wipe(masterKey);
      throw new VaultCryptoError("malformed");
    }
    if (wrapsMade > MAX_WRAPS) {
      wipe(masterKey);
      throw new VaultCryptoError("wrap-bound");
    }
    const material = await crypto.subtle.importKey("raw", masterKey, "HKDF", false, ["deriveKey"]);
    wipe(masterKey);
    const key = await crypto.subtle.deriveKey(
      { name: "HKDF", hash: "SHA-256", salt: new Uint8Array(0), info: utf8(LABEL.unlockWrap) },
      material,
      { name: "AES-GCM", length: 256 },
      false,
      ["wrapKey", "unwrapKey"],
    );
    return new UnlockKey(key, wrapsMade);
  }

  get wrapsMade(): number {
    return this.#wrapsMade;
  }

  /** Wrap a freshly generated, still extractable private key; refused once 2^20 wraps were made. */
  async wrap(privateKey: CryptoKey, context: MemberKeyContext): Promise<Bytes> {
    if (this.#wrapsMade >= MAX_WRAPS) {
      throw new VaultCryptoError("wrap-bound");
    }
    this.#wrapsMade += 1;
    const nonce = random(NONCE_BYTES);
    const sealed = await crypto.subtle.wrapKey("pkcs8", privateKey, this.#key, {
      name: "AES-GCM",
      iv: nonce,
      additionalData: associatedData(context),
    });
    return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.memberKey), context.keyId, nonce, new Uint8Array(sealed));
  }

  /** Seal the company owner's Ed25519 public key under UK, to be pinned in this member's key set; one more wrap. */
  async sealPin(ownerPublic: CryptoKey, pin: PinContext): Promise<{ nonce: Bytes; sealed: Bytes }> {
    if (this.#wrapsMade >= MAX_WRAPS) {
      throw new VaultCryptoError("wrap-bound");
    }
    this.#wrapsMade += 1;
    const nonce = random(NONCE_BYTES);
    const sealed = await crypto.subtle.wrapKey("raw", ownerPublic, this.#key, {
      name: "AES-GCM",
      iv: nonce,
      additionalData: pinData(pin),
    });
    return { nonce, sealed: new Uint8Array(sealed) };
  }

  /** Open the pinned owner key; a pin sealed for another member, company or key set is `unauthentic`. */
  async openPin(pin: PinContext, nonce: Bytes, sealed: Bytes): Promise<CryptoKey> {
    if (nonce.length !== NONCE_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    try {
      return await crypto.subtle.unwrapKey(
        "raw",
        sealed,
        this.#key,
        { name: "AES-GCM", iv: nonce, additionalData: pinData(pin) },
        { name: "Ed25519" },
        true,
        ["verify"],
      );
    } catch {
      throw new VaultCryptoError("unauthentic");
    }
  }

  /** Unwrap into a key, non-extractable unless asked (escrow only): X25519 for agreement, Ed25519 for signing. */
  async unwrap(context: MemberKeyContext, wrapped: Bytes, extractable = false): Promise<CryptoKey> {
    const reader = new Reader(wrapped);
    if (reader.integer(1) !== FORMAT_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    if (reader.integer(1) !== ALGORITHM.memberKey) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    const keyId = reader.take(KEY_ID_BYTES);
    const nonce = reader.take(NONCE_BYTES);
    const sealed = reader.rest();
    if (!sameBytes(keyId, context.keyId)) {
      throw new VaultCryptoError("key-mismatch");
    }
    const agreement = context.kind === "agreement";
    try {
      return await crypto.subtle.unwrapKey(
        "pkcs8",
        sealed,
        this.#key,
        { name: "AES-GCM", iv: nonce, additionalData: associatedData(context) },
        { name: agreement ? "X25519" : "Ed25519" },
        extractable,
        agreement ? ["deriveBits"] : ["sign"],
      );
    } catch {
      throw new VaultCryptoError("unauthentic");
    }
  }
}
