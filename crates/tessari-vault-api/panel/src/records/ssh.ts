// SSH key fields: Ed25519 only in v1. A key is generated or imported in OpenSSH's own formats: the unencrypted
// `openssh-key-v1` private key (OpenSSH PROTOCOL.key), the `ssh-ed25519` public line (RFC 8709) and the `SHA256:`
// fingerprint `ssh-keygen -l` prints. The private key is a field value like any other and is sealed with the record.

import { Reader, concat, integer, random, sameBytes, utf8, wipe, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { ed25519PrivateFromSeed } from "../crypto/keys.ts";

export interface SshKey {
  /** The OpenSSH private key file's text. */
  privateKey: string;
  /** `ssh-ed25519 <base64> <comment>`, as `authorized_keys` takes it. */
  publicKey: string;
  /** `SHA256:` and the unpadded base64 of the public blob's digest. */
  fingerprint: string;
}

const MAGIC = concat(utf8("openssh-key-v1"), Uint8Array.of(0));
const KEY_TYPE = "ssh-ed25519";
const NONE = "none";
const KEY_BYTES = 32;
const BLOCK = 8;
const LINE = 70;
const BEGIN = "-----BEGIN OPENSSH PRIVATE KEY-----";
const END = "-----END OPENSSH PRIVATE KEY-----";

/** An SSH wire string: a four-byte length, then the bytes. */
function sshString(bytes: Uint8Array): Bytes {
  return concat(integer(bytes.length, 4), bytes);
}

function base64(bytes: Uint8Array): string {
  return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
}

function fromBase64(text: string): Bytes {
  try {
    return Uint8Array.from(atob(text), (character) => character.charCodeAt(0));
  } catch {
    throw new VaultCryptoError("malformed");
  }
}

function text(reader: Reader): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(reader.field());
  } catch {
    throw new VaultCryptoError("malformed");
  }
}

async function describe(publicKey: Bytes, comment: string, privateKey: string): Promise<SshKey> {
  const blob = concat(sshString(utf8(KEY_TYPE)), sshString(publicKey));
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", blob));
  return {
    privateKey,
    publicKey: [KEY_TYPE, base64(blob), comment].filter((part) => part !== "").join(" "),
    fingerprint: `SHA256:${base64(digest).replace(/=+$/, "")}`,
  };
}

/** The Ed25519 public key a seed belongs to. */
async function publicOf(seed: Bytes): Promise<Bytes> {
  const key = await ed25519PrivateFromSeed(seed.slice(), true);
  const jwk = await crypto.subtle.exportKey("jwk", key);
  return fromBase64((jwk.x ?? "").replace(/-/g, "+").replace(/_/g, "/").padEnd(44, "="));
}

function privateFile(seed: Bytes, publicKey: Bytes, comment: string): string {
  const check = random(4);
  let section = concat(
    check,
    check,
    sshString(utf8(KEY_TYPE)),
    sshString(publicKey),
    sshString(concat(seed, publicKey)),
    sshString(utf8(comment)),
  );
  const padding = Uint8Array.from({ length: (BLOCK - (section.length % BLOCK)) % BLOCK }, (_, index) => index + 1);
  section = concat(section, padding);
  const body = concat(
    MAGIC,
    sshString(utf8(NONE)),
    sshString(utf8(NONE)),
    sshString(new Uint8Array(0)),
    integer(1, 4),
    sshString(concat(sshString(utf8(KEY_TYPE)), sshString(publicKey))),
    sshString(section),
  );
  const encoded = base64(body);
  wipe(section, body);
  const lines = encoded.match(new RegExp(`.{1,${LINE}}`, "g")) ?? [];
  return `${BEGIN}\n${lines.join("\n")}\n${END}\n`;
}

/** A new Ed25519 key in OpenSSH form. */
export async function generateSshKey(comment: string): Promise<SshKey> {
  const pair = await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
  if (!("privateKey" in pair)) {
    throw new VaultCryptoError("malformed");
  }
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
  const seed = pkcs8.slice(pkcs8.length - KEY_BYTES);
  const publicKey = new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey));
  const key = await describe(publicKey, comment, privateFile(seed, publicKey, comment));
  wipe(pkcs8, seed);
  return key;
}

/** Read an unencrypted OpenSSH Ed25519 private key; every copy of the public key must belong to the private one. */
export async function importSshKey(pem: string): Promise<SshKey> {
  const lines = pem.trim().split(/\r?\n/);
  if (lines[0] !== BEGIN || lines.at(-1) !== END) {
    throw new VaultCryptoError("malformed");
  }
  const reader = new Reader(fromBase64(lines.slice(1, -1).join("")));
  if (!sameBytes(reader.take(MAGIC.length), MAGIC)) {
    throw new VaultCryptoError("malformed");
  }
  if (text(reader) !== NONE || text(reader) !== NONE) {
    throw new VaultCryptoError("unknown-algorithm");
  }
  if (reader.field().length !== 0 || reader.integer(4) !== 1) {
    throw new VaultCryptoError("malformed");
  }
  const blob = new Reader(reader.field());
  const section = new Reader(reader.field());
  reader.end();
  if (text(blob) !== KEY_TYPE) {
    throw new VaultCryptoError("unknown-algorithm");
  }
  const publicKey = blob.field();
  blob.end();
  if (section.integer(4) !== section.integer(4) || text(section) !== KEY_TYPE) {
    throw new VaultCryptoError("malformed");
  }
  const repeated = section.field();
  const pair = section.field();
  const comment = text(section);
  const padding = section.rest();
  if (publicKey.length !== KEY_BYTES || pair.length !== 2 * KEY_BYTES || !padding.every((byte, index) => byte === index + 1)) {
    throw new VaultCryptoError("malformed");
  }
  const seed = pair.slice(0, KEY_BYTES);
  const matches =
    sameBytes(repeated, publicKey) && sameBytes(pair.slice(KEY_BYTES), publicKey) && sameBytes(await publicOf(seed), publicKey);
  wipe(seed, pair);
  if (!matches) {
    throw new VaultCryptoError("key-mismatch");
  }
  return describe(publicKey, comment, pem);
}
