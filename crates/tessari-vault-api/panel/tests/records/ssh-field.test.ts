// Pasting an SSH key that can't be kept says why: not an OpenSSH private key, a key behind a passphrase or of another
// kind, or a file whose public half belongs to a different key.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { VaultCryptoError } from "../../src/crypto/errors.ts";
import { sshImportProblem } from "../../src/records/ssh-field.ts";

describe("pasting an SSH key", () => {
  it("says why each refused key can't be kept", () => {
    assert.match(sshImportProblem(new VaultCryptoError("malformed")), /BEGIN OPENSSH PRIVATE KEY/);
    assert.match(sshImportProblem(new VaultCryptoError("unknown-algorithm")), /passphrase/);
    assert.match(sshImportProblem(new VaultCryptoError("unknown-algorithm")), /Ed25519/);
    assert.match(sshImportProblem(new VaultCryptoError("key-mismatch")), /doesn't match/);
  });

  it("does not pass off another failure as a bad paste", () => {
    assert.match(sshImportProblem(new Error("boom")), /couldn't read/);
  });
});
