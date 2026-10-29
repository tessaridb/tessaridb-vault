// What the edit form says when a pasted SSH key can't be kept. No DOM here, so it is tested.

import { VaultCryptoError } from "../crypto/errors.ts";

/** Why `problem`, thrown while reading a pasted private key, means the key can't be kept, in words. */
export function sshImportProblem(problem: unknown): string {
  if (!(problem instanceof VaultCryptoError)) {
    return "The page couldn't read this key. Try pasting it again.";
  }
  switch (problem.reason) {
    case "unknown-algorithm":
      return "Only an Ed25519 key without a passphrase can be kept. Remove the passphrase with ssh-keygen -p, or make a new key here.";
    case "key-mismatch":
      return "The public key in this file doesn't match its private key, so it isn't kept.";
    default:
      return "Paste the whole private key file, from -----BEGIN OPENSSH PRIVATE KEY----- to its END line.";
  }
}
