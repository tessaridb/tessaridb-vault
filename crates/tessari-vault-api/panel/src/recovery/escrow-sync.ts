// Escrow at unlock (C7): once the owner's chain vouches a company recovery key, every member's browser wraps their
// private keys to it, so an owner can restore them later. The keys are exportable only while the unlock key is in hand,
// so whether an escrow is owed is asked BEFORE the unlock password is derived, and the export happens only then.
// Before the keys open, the chain is checked against the owner key the server names; after, against the key the
// member's own key set pins, and the escrow goes only to the recovery key that pinned chain vouches for.

import { call, ignored } from "../api.ts";
import { readChain, type Chain } from "../chain-model.ts";
import { wipe, type Bytes } from "../crypto/bytes.ts";
import { openKeySet, openKeySetWithEscrow, type OpenKeySet, type StoredKeySet } from "../crypto/key-set.ts";
import { wrapToVouched } from "../crypto/recipients.ts";
import { verifyChain, type RecoveryKey } from "../crypto/statements.ts";
import { keyIdBytes } from "../unlock-model.ts";
import { escrowBody, escrowWanted, readOwnEscrow, readRecoveryKey } from "./recovery-model.ts";

/** An escrow owed: the chain read before unlocking, and the recovery key it named. */
export interface EscrowOwed {
  readonly chain: Chain;
  readonly vouched: RecoveryKey;
}

const companyPath = (company: string): string => `/api/companies/${encodeURIComponent(company)}`;

/** Whether this member owes `company` an escrow; null when not, or when it cannot be told now (asked again next time). */
export async function escrowOwed(company: string): Promise<EscrowOwed | null> {
  const [chain, own] = await Promise.all([call("GET", `${companyPath(company)}/statements`, readChain), call("GET", `${companyPath(company)}/keys/me/escrow`, readOwnEscrow)]);
  if (!chain.ok || chain.value.ownerSigning === null || (!own.ok && own.status !== 404)) {
    return null;
  }
  try {
    const vouched = (await verifyChain(company, chain.value.ownerSigning, chain.value.statements)).recoveryKey();
    return vouched !== undefined && escrowWanted(own.ok ? own.value : null, vouched) ? { chain: chain.value, vouched } : null;
  } catch {
    return null;
  }
}

/**
 * Wraps `escrow` to the recovery key the chain vouches from `keys`' pinned owner key and keeps it; `escrow` is wiped
 * here whatever happens, and a failure is left for the next unlock to try again.
 */
export async function keepEscrow(company: string, keys: OpenKeySet, owed: EscrowOwed | null, escrow: Bytes | null): Promise<boolean> {
  if (escrow === null) {
    return false;
  }
  try {
    if (owed === null) {
      return false;
    }
    const chain = await verifyChain(company, keys.ownerSigningPublic, owed.chain.statements);
    const vouched = chain.recoveryKey();
    const recoveryKeyId = vouched === undefined ? null : keyIdBytes(vouched.keyId);
    const listed = await call("GET", `${companyPath(company)}/recovery-key`, readRecoveryKey);
    if (!listed.ok || vouched === undefined || recoveryKeyId === null || listed.value.keyId !== vouched.keyId) {
      return false;
    }
    const context = { company, wrappedKind: "member-recovery", wrappedKeyId: keys.keyId, recipientKind: "recovery", recipientKeyId: recoveryKeyId } as const;
    const wrap = await wrapToVouched(chain, { kind: "recovery" }, listed.value.public, context, escrow);
    return (await call("PUT", `${companyPath(company)}/keys/me/escrow`, ignored, { body: escrowBody(vouched.keyId, wrap) })).ok;
  } catch {
    return false;
  } finally {
    wipe(escrow);
  }
}

/** Opens `stored` with the unlock password and, when an escrow is owed, escrows the keys before answering. */
export async function openAndEscrow(module: WebAssembly.Module, password: string, account: string, company: string, stored: StoredKeySet): Promise<OpenKeySet> {
  const owed = await escrowOwed(company);
  if (owed === null) {
    return openKeySet(module, password, account, company, stored);
  }
  const { open, escrow } = await openKeySetWithEscrow(module, password, account, company, stored);
  await keepEscrow(company, open, owed, escrow);
  return open;
}
