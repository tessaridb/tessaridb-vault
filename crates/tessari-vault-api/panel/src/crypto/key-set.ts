// A member's key set in one company: an X25519 agreement key and an Ed25519 signing key, made in this browser and
// wrapped under the unlock key, with the company owner's public key pinned beside them (Q-VA-36: the root every
// statement chain is checked from, so a server that later lists another owner key gains nothing). The server keeps it in
// its opaque `wrapped` field as `[2][wraps made 4][field agreement wrap][field signing wrap][pin nonce 12][field pin]`.
// Each wrap and the pin authenticate their own account, company and key id, so the count is the one value the server
// could change unseen; lowering it lets one unlock key make more wraps than its bound, which stays far inside the nonce
// limit because each unlock password makes a new unlock key from a new salt, and that key makes three wraps.

import { Reader, concat, field, integer, random, sameBytes, wipe, type Bytes } from "./bytes.ts";
import { escrowBytes, readEscrow } from "./escrow.ts";
import { VaultCryptoError } from "./errors.ts";
import { ed25519PublicFromBytes, generatePair, publicBytes, x25519, x25519PublicFromBytes } from "./keys.ts";
import { UnlockKey, deriveMasterKey, type KdfParams, type MemberKeyContext } from "./unlock.ts";

/** Argon2id for a new unlock password: the floor the module enforces (RFC 9106 second option). */
export const NEW_KDF: KdfParams = { memoryKib: 65_536, passes: 3, lanes: 4 };

const SALT_BYTES = 16;
const KEY_ID_BYTES = 16;
const PIN_NONCE_BYTES = 12;
/** The key-set layout's own version: 2 added the pinned owner key. */
const KEY_SET_VERSION = 2;

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
  /** The company owner's Ed25519 public key, as pinned when this key set was made. */
  readonly ownerSigningPublic: Bytes;
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
interface Parsed {
  readonly wrapsMade: number;
  readonly agreementWrap: Bytes;
  readonly signingWrap: Bytes;
  readonly pinNonce: Bytes;
  readonly pin: Bytes;
}

function parse(stored: StoredKeySet): Parsed {
  const reader = new Reader(stored.wrapped);
  if (reader.integer(1) !== KEY_SET_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  const wrapsMade = reader.integer(4);
  const agreementWrap = reader.field();
  const signingWrap = reader.field();
  const pinNonce = reader.take(PIN_NONCE_BYTES);
  const pin = reader.field();
  reader.end();
  return { wrapsMade, agreementWrap, signingWrap, pinNonce, pin };
}

async function openWith(key: UnlockKey, account: string, company: string, stored: StoredKeySet): Promise<OpenKeySet> {
  const { agreementWrap, signingWrap, pinNonce, pin } = parse(stored);
  const owner = await key.openPin({ account, company, keyId: stored.keyId }, pinNonce, pin);
  return {
    keyId: stored.keyId,
    agreementPrivate: await key.unwrap(context(account, company, "agreement", stored.keyId), agreementWrap),
    signingPrivate: await key.unwrap(context(account, company, "signing", stored.keyId), signingWrap),
    agreementPublic: stored.agreementPublic,
    signingPublic: stored.signingPublic,
    ownerSigningPublic: await publicBytes(owner),
  };
}

/** The escrow of two extractable private keys and the pinned owner key; the PKCS #8 copies are wiped once packed. */
async function escrowOf(agreement: CryptoKey, signing: CryptoKey, ownerSigningPublic: Bytes): Promise<Bytes> {
  const agreementBytes = new Uint8Array(await crypto.subtle.exportKey("pkcs8", agreement));
  const signingBytes = new Uint8Array(await crypto.subtle.exportKey("pkcs8", signing));
  try {
    return escrowBytes({ agreement: agreementBytes, signing: signingBytes, ownerSigningPublic });
  } finally {
    wipe(agreementBytes, signingBytes);
  }
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

/**
 * Opens `stored` as `openKeySet` does and also answers its escrow, from a second, extractable unwrap that lives only
 * inside this call. The caller wraps the escrow to the recovery key and wipes it.
 */
export async function openKeySetWithEscrow(
  module: WebAssembly.Module,
  password: string,
  account: string,
  company: string,
  stored: StoredKeySet,
): Promise<{ readonly open: OpenKeySet; readonly escrow: Bytes }> {
  const parsed = parse(stored);
  const key = await unlockKey(module, password, stored.salt, stored.kdf, parsed.wrapsMade);
  const open = await openWith(key, account, company, stored);
  const agreement = await key.unwrap(context(account, company, "agreement", stored.keyId), parsed.agreementWrap, true);
  const signing = await key.unwrap(context(account, company, "signing", stored.keyId), parsed.signingWrap, true);
  return { open, escrow: await escrowOf(agreement, signing, open.ownerSigningPublic) };
}

interface Halves {
  readonly agreementPrivate: CryptoKey;
  readonly signingPrivate: CryptoKey;
  readonly agreementPublic: Bytes;
  readonly signingPublic: Bytes;
}

/** `halves` wrapped under a new unlock key from `password` and a new salt, the owner key pinned, packed and opened. */
async function sealed(
  module: WebAssembly.Module,
  password: string,
  account: string,
  company: string,
  keyId: Bytes,
  halves: Halves,
  ownerSigningPublic: Bytes,
): Promise<{ readonly stored: StoredKeySet; readonly open: OpenKeySet }> {
  const salt = random(SALT_BYTES);
  const key = await unlockKey(module, password, salt, NEW_KDF, 0);
  const agreementWrap = await key.wrap(halves.agreementPrivate, context(account, company, "agreement", keyId));
  const signingWrap = await key.wrap(halves.signingPrivate, context(account, company, "signing", keyId));
  const pin = await key.sealPin(await ed25519PublicFromBytes(ownerSigningPublic), { account, company, keyId });
  const stored: StoredKeySet = {
    keyId,
    agreementPublic: halves.agreementPublic,
    signingPublic: halves.signingPublic,
    wrapped: concat(Uint8Array.of(KEY_SET_VERSION), integer(key.wrapsMade, 4), field(agreementWrap), field(signingWrap), pin.nonce, field(pin.sealed)),
    salt,
    kdf: NEW_KDF,
  };
  // The keys kept open are the non-extractable ones the wraps yield, which also proves the stored set opens.
  return { stored, open: await openWith(key, account, company, stored) };
}

/**
 * A new key set for `account` in `company` under a new unlock password, ready to send, and opened. `ownerSigningPublic`
 * is the company owner's key to pin: the person's own new signing key when they are the first owner (`"self"`). With
 * `forEscrow` it also answers the new keys' escrow, which the caller wraps to the recovery key and wipes.
 */
export async function createKeySet(
  module: WebAssembly.Module,
  password: string,
  account: string,
  company: string,
  ownerSigningPublic: Bytes | "self",
  forEscrow = false,
): Promise<{ readonly stored: StoredKeySet; readonly open: OpenKeySet; readonly escrow: Bytes | null }> {
  const agreement = await generatePair("X25519");
  const signing = await generatePair("Ed25519");
  const signingPublic = await publicBytes(signing.publicKey);
  const owner = ownerSigningPublic === "self" ? signingPublic : ownerSigningPublic;
  const halves = { agreementPrivate: agreement.privateKey, signingPrivate: signing.privateKey, agreementPublic: await publicBytes(agreement.publicKey), signingPublic };
  const made = await sealed(module, password, account, company, random(KEY_ID_BYTES), halves, owner);
  return { ...made, escrow: forEscrow ? await escrowOf(agreement.privateKey, signing.privateKey, owner) : null };
}

/** Whether `agreement` and `signing` are the private halves of `stored`'s public keys. */
async function sameKeys(agreement: CryptoKey, signing: CryptoKey, stored: StoredKeySet): Promise<boolean> {
  const peer = await generatePair("X25519", false);
  const mine = await x25519(agreement, peer.publicKey);
  const theirs = await x25519(peer.privateKey, await x25519PublicFromBytes(stored.agreementPublic));
  const probe = random(32);
  const signature = new Uint8Array(await crypto.subtle.sign("Ed25519", signing, probe));
  const signed = await crypto.subtle.verify("Ed25519", await ed25519PublicFromBytes(stored.signingPublic), signature, probe);
  return sameBytes(mine, theirs) && signed;
}

/**
 * The member's key set `stored` restored from `escrow` (handed back by an owner) under a new unlock password: the same
 * key id and keys, the same owner key pinned. Keys that are not `stored`'s are `key-mismatch`; the escrow is wiped.
 */
export async function restoreKeySet(
  module: WebAssembly.Module,
  password: string,
  account: string,
  company: string,
  stored: StoredKeySet,
  escrow: Bytes,
): Promise<{ readonly stored: StoredKeySet; readonly open: OpenKeySet }> {
  const opened = readEscrow(escrow);
  try {
    const agreementPrivate = await crypto.subtle.importKey("pkcs8", opened.agreement, { name: "X25519" }, true, ["deriveBits"]);
    const signingPrivate = await crypto.subtle.importKey("pkcs8", opened.signing, { name: "Ed25519" }, true, ["sign"]);
    if (!(await sameKeys(agreementPrivate, signingPrivate, stored))) {
      throw new VaultCryptoError("key-mismatch");
    }
    const halves = { agreementPrivate, signingPrivate, agreementPublic: stored.agreementPublic, signingPublic: stored.signingPublic };
    return await sealed(module, password, account, company, stored.keyId, halves, opened.ownerSigningPublic);
  } finally {
    wipe(opened.agreement, opened.signing, escrow);
  }
}
