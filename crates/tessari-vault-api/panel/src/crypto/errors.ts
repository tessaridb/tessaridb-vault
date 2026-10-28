// The one error the crypto module raises. Its reason is a closed set, so the panel can say why a record is unreadable
// without parsing a message, and a GCM failure is one reason whether the tag, the associated data or the key was wrong.

export type Refusal =
  | "unknown-version"
  | "unknown-algorithm"
  | "unknown-key"
  | "key-mismatch"
  | "malformed"
  | "unauthentic"
  | "wrap-bound"
  | "unvouched-signer"
  | "chain-broken"
  | "bad-signature"
  | "weak-shared-secret"
  | "kdf-refused";

export class VaultCryptoError extends Error {
  readonly reason: Refusal;

  constructor(reason: Refusal) {
    super(`refused: ${reason}`);
    this.name = "VaultCryptoError";
    this.reason = reason;
  }
}
