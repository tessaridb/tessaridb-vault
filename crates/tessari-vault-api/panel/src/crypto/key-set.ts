// A member's key set in one company: an X25519 agreement key and an Ed25519 signing key, made in this browser and
// wrapped under the unlock key. The server keeps the two wraps in its opaque `wrapped` field as
// `[version][wraps made 4][field agreement wrap][field signing wrap]`. Each wrap authenticates its own kind, account,
// company and key id, so the count is the one value the server could change unseen; lowering it lets one unlock key make
// more wraps than its bound, which stays far inside the nonce limit because each unlock password makes a new unlock key
// from a new salt, and that key makes two wraps.

import { Reader, concat, field, integer, random, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { generatePair, publicBytes } from "./keys.ts";
import { FORMAT_VERSION } from "./labels.ts";
import { UnlockKey, deriveMasterKey, type KdfParams, type MemberKeyContext } from "./unlock.ts";

/** Argon2id for a new unlock password: the floor the module enforces (RFC 9106 second option). */
export const NEW_KDF: KdfParams = { memoryKib: 65_536, passes: 3, lanes: 4 };

const SALT_BYTES = 16;
const KEY_ID_BYTES = 16;

/** The key set as the server stores it. */
export interface StoredKeySet {
  readonly keyId: Bytes;
  readonly agreementPublic: Bytes;
  readonly signingPublic: Bytes;
  readonly wrapped: Bytes;
  readonly salt: Bytes;
  readonly kdf: KdfParams;
}

/** A key set opened in this browser: private halves non-extractable, public halves as bytes. */
export interface OpenKeySet {
  readonly keyId: Bytes;
  readonly agreementPrivate: CryptoKey;
  readonly signingPrivate: CryptoKey;
  readonly agreementPublic: Bytes;
  readonly signingPublic: Bytes;
}

const context = (account: string, company: string, kind: "agreement" | "signing", keyId: Bytes): MemberKeyContext => ({
  account,
  company,
  kind,
  keyId,
});

async function unlockKey(module: WebAssembly.Module, password: string, salt: Bytes, kdf: KdfParams, wrapsMade: number): Promise<UnlockKey> {
  return UnlockKey.fromMasterKey(await deriveMasterKey(module, password, salt, kdf), wrapsMade);
}

/** The two wraps of `stored` and the count, or `malformed` / `unknown-version` when it is not a key set of this format. */
function parse(stored: StoredKeySet): { readonly wrapsMade: number; readonly agreementWrap: Bytes; readonly signingWrap: Bytes } {
  const reader = new Reader(stored.wrapped);
  if (reader.integer(1) !== FORMAT_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  const wrapsMade = reader.integer(4);
  const agreementWrap = reader.field();
  const signingWrap = reader.field();
  reader.end();
  return { wrapsMade, agreementWrap, signingWrap };
}

async function openWith(key: UnlockKey, account: string, company: string, stored: StoredKeySet): Promise<OpenKeySet> {
  const { agreementWrap, signingWrap } = parse(stored);
  return {
    keyId: stored.keyId,
    agreementPrivate: await key.unwrap(context(account, company, "agreement", stored.keyId), agreementWrap),
    signingPrivate: await key.unwrap(context(account, company, "signing", stored.keyId), signingWrap),
    agreementPublic: stored.agreementPublic,
    signingPublic: stored.signingPublic,
  };
}

/** Opens `stored` with the unlock password; a wrong password is `unauthentic`, a damaged set `malformed`. */
export async function openKeySet(
  module: WebAssembly.Module,
  password: string,
  account: string,
  company: string,
  stored: StoredKeySet,
): Promise<OpenKeySet> {
  const { wrapsMade } = parse(stored);
  return openWith(await unlockKey(module, password, stored.salt, stored.kdf, wrapsMade), account, company, stored);
}

/** A new key set for `account` in `company` under a new unlock password, ready to send, and opened. */
export async function createKeySet(
  module: WebAssembly.Module,
  password: string,
  account: string,
  company: string,
): Promise<{ readonly stored: StoredKeySet; readonly open: OpenKeySet }> {
  const salt = random(SALT_BYTES);
  const keyId = random(KEY_ID_BYTES);
  const key = await unlockKey(module, password, salt, NEW_KDF, 0);
  const agreement = await generatePair("X25519");
  const signing = await generatePair("Ed25519");
  const agreementWrap = await key.wrap(agreement.privateKey, context(account, company, "agreement", keyId));
  const signingWrap = await key.wrap(signing.privateKey, context(account, company, "signing", keyId));
  const stored: StoredKeySet = {
    keyId,
    agreementPublic: await publicBytes(agreement.publicKey),
    signingPublic: await publicBytes(signing.publicKey),
    wrapped: concat(Uint8Array.of(FORMAT_VERSION), integer(key.wrapsMade, 4), field(agreementWrap), field(signingWrap)),
    salt,
    kdf: NEW_KDF,
  };
  // The generated private halves are extractable; the keys kept open are the non-extractable ones the wraps yield,
  // which also proves the stored set opens before it is sent.
  return { stored, open: await openWith(key, account, company, stored) };
}
