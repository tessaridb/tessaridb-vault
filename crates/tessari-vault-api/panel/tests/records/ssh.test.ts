// SSH key fields: an Ed25519 key generated in the browser in OpenSSH's own formats, and an OpenSSH Ed25519 key
// imported with its fingerprint. The oracle is the system `ssh-keygen`; without it these tests fail rather than skip.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, describe, it } from "node:test";

import { generateSshKey, importSshKey } from "../../src/records/ssh.ts";
import { refusedWith } from "../crypto/support.ts";

const DIRECTORY = mkdtempSync(join(tmpdir(), "vault-ssh-"));
after(() => rmSync(DIRECTORY, { recursive: true, force: true }));

function keygen(...args: string[]): string {
  return execFileSync("ssh-keygen", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
}

/** `ssh-keygen -l` prints "256 SHA256:<digest> comment (ED25519)"; the fingerprint is the second word. */
function fingerprintOf(path: string): string {
  return keygen("-l", "-f", path).split(" ")[1] ?? "";
}

/** The key type and key, without the comment. */
function keyWords(line: string): string {
  return line.split(" ").slice(0, 2).join(" ");
}

let serial = 0;
function writeKey(contents: string): string {
  serial += 1;
  const path = join(DIRECTORY, `key-${serial}`);
  writeFileSync(path, contents, { mode: 0o600 });
  return path;
}

describe("SSH Ed25519 keys", () => {
  it("generates a key ssh-keygen reads back to the same public key and fingerprint", async () => {
    const key = await generateSshKey("deploy@vault");
    assert.match(key.publicKey, /^ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI[A-Za-z0-9+/]+ deploy@vault$/);
    const path = writeKey(key.privateKey);
    assert.equal(keyWords(keygen("-y", "-f", path)), keyWords(key.publicKey));
    assert.equal(fingerprintOf(path), key.fingerprint);
  });

  it("imports a key ssh-keygen generated, with ssh-keygen's fingerprint", async () => {
    const path = join(DIRECTORY, "from-keygen");
    keygen("-q", "-t", "ed25519", "-N", "", "-C", "ops@example", "-f", path);
    const key = await importSshKey(readFileSync(path, "utf8"));
    assert.equal(keyWords(key.publicKey), keyWords(readFileSync(`${path}.pub`, "utf8")));
    assert.equal(key.fingerprint, fingerprintOf(`${path}.pub`));
    assert.ok(key.publicKey.endsWith(" ops@example"));
  });

  it("refuses a passphrase-protected key and a key of another type", async () => {
    const protectedPath = join(DIRECTORY, "protected");
    keygen("-q", "-t", "ed25519", "-N", "a passphrase", "-f", protectedPath);
    await refusedWith(importSshKey(readFileSync(protectedPath, "utf8")), "unknown-algorithm");
    const ecdsaPath = join(DIRECTORY, "ecdsa");
    keygen("-q", "-t", "ecdsa", "-N", "", "-f", ecdsaPath);
    await refusedWith(importSshKey(readFileSync(ecdsaPath, "utf8")), "unknown-algorithm");
  });

  it("refuses a key whose public half does not belong to its private half", async () => {
    const mine = await generateSshKey("");
    const other = await generateSshKey("");
    // Swap in another key's public blob where the file carries it the first time (outside the private section).
    const body = (pem: string): Uint8Array =>
      Uint8Array.from(atob(pem.split("\n").filter((line) => !line.startsWith("-----")).join("")), (c) => c.charCodeAt(0));
    const mixed = body(mine.privateKey);
    const donor = body(other.privateKey);
    // The private section repeats the public key: magic 15 + cipher 8 + kdf 8 + options 4 + count 4 + public blob 55
    // + section length 4 + check ints 8 + key type 15 + key length 4 = byte 125.
    const at = 15 + 8 + 8 + 4 + 4 + 55 + 4 + 8 + 15 + 4;
    mixed.set(donor.slice(at, at + 32), at);
    const pem = `-----BEGIN OPENSSH PRIVATE KEY-----\n${btoa(String.fromCharCode(...mixed))}\n-----END OPENSSH PRIVATE KEY-----\n`;
    await refusedWith(importSshKey(pem), "key-mismatch");
  });

  it("refuses text that is not an OpenSSH private key", async () => {
    await refusedWith(importSshKey("ssh-ed25519 AAAA not a private key"), "malformed");
  });
});
