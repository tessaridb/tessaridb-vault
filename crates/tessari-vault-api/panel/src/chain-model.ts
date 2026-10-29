// The company's vouching chain as `GET /api/companies/{company}/statements` answers it, which owner key a new key set
// pins (Q-VA-36: trusted when the key set is made, then held under the unlock key), and the first statement an owner's
// browser owes an empty chain. No DOM here, so all of it is tested.

import { decode, encode } from "./base64url.ts";
import { sameBytes, type Bytes } from "./crypto/bytes.ts";
import type { Statement } from "./crypto/statements.ts";
import type { Role } from "./unlock-model.ts";

const OWNER_KEY_BYTES = 32;

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";

/** The chain: the owner key its first statement pinned, and every statement in order; both empty before the first. */
export interface Chain {
  readonly ownerSigning: Bytes | null;
  readonly statements: readonly Statement[];
}

export function readChain(v: unknown): Chain | null {
  if (!isRecord(v) || !Array.isArray(v["statements"])) {
    return null;
  }
  const pinned = v["owner_signing"];
  const ownerSigning = pinned === null ? null : isText(pinned) ? decode(pinned) : undefined;
  if (ownerSigning === undefined || (ownerSigning !== null && ownerSigning.length !== OWNER_KEY_BYTES)) {
    return null;
  }
  const statements: Statement[] = [];
  for (const [index, item] of v["statements"].entries()) {
    if (!isRecord(item) || item["seq"] !== index + 1 || !isText(item["bytes"]) || !isText(item["signature"])) {
      return null;
    }
    const [bytes, signature] = [decode(item["bytes"]), decode(item["signature"])];
    if (bytes === null || signature === null) {
      return null;
    }
    statements.push({ bytes, signature });
  }
  // The server pins the owner key with the first statement and only then, so one without the other is not a chain.
  return (ownerSigning === null) === (statements.length === 0) ? { ownerSigning, statements } : null;
}

/** The owner key to pin: the chain's; the person's own when an owner finds no chain yet; none for anyone else. */
export function ownerPinFor(chain: Chain, role: Role | null): Bytes | "self" | "not-yet" {
  if (chain.ownerSigning !== null) {
    return chain.ownerSigning;
  }
  return role === "owner" ? "self" : "not-yet";
}

/** Whether these keys owe the chain its first statement: they pinned themselves and the chain has not begun. */
export const needsGenesis = (chain: Chain, keys: { readonly signingPublic: Bytes; readonly ownerSigningPublic: Bytes }): boolean =>
  chain.ownerSigning === null && sameBytes(keys.ownerSigningPublic, keys.signingPublic);

/** The body `POST /api/companies/{company}/statements` takes for the first statement, which pins the owner key. */
export const genesisBody = (statement: Statement, ownerSigning: Bytes): Readonly<Record<string, unknown>> => ({
  seq: 1,
  bytes: encode(statement.bytes),
  signature: encode(statement.signature),
  owner_signing: encode(ownerSigning),
});
