// An owner's two acts on the recovery page. Making the recovery key: the statement vouching for it goes first, so a
// key the chain does not name can never be the company's one recovery key; then the key with its owners' wraps.
// Giving the recovery key to owners still waiting: the owner's own wrap opened here, the key wrapped to each owner the
// chain vouches for and wiped. Handing a member's keys over: the owner's own wrap of the recovery key and the member's escrow, both checked to name
// the key the owner's chain vouches for, opened here and wrapped to the member's one-time handoff key.

import { call, ignored, type Failure } from "../api.ts";
import type { Chain } from "../chain-model.ts";
import { handOver } from "../crypto/recovery.ts";
import type { Vouched } from "../crypto/statements.ts";
import { groupKeyBody, wrapsBody, type PendingKey } from "../groups/group-key.ts";
import { statementBody } from "../members/members-model.ts";
import type { DirectoryKey } from "../records/shares-model.ts";
import { companyPath } from "../records/load.ts";
import { keyIdBytes } from "../unlock-model.ts";
import type { Unlocked } from "../vault.ts";
import { handoffBody, readWrap, type OpenRequest } from "./recovery-model.ts";
import { newRecoveryKey, wrapsForOwners, type OwnerKey } from "./recovery-key.ts";

const refused = (code: string): Failure => ({ ok: false, status: 0, code, message: "" });

/** Makes the company's recovery key, wrapped to `owners`; null once kept, else what went wrong. */
export async function makeRecoveryKey(held: Unlocked, chain: Chain, vouched: Vouched, owners: readonly OwnerKey[]): Promise<Failure | null> {
  const signer = { privateKey: held.keys.signingPrivate, publicBytes: held.keys.signingPublic };
  let made;
  try {
    made = await newRecoveryKey(held.company, signer, held.keys.ownerSigningPublic, chain.statements, vouched, owners);
  } catch {
    return refused("unvouched-signer");
  }
  const path = companyPath(held);
  const vouching = await call("POST", `${path}/statements`, ignored, { body: statementBody(chain.statements.length + 1, made.statement) });
  if (!vouching.ok) {
    return vouching;
  }
  const kept = await call("POST", `${path}/recovery-key`, ignored, { body: groupKeyBody(made) });
  return kept.ok ? null : kept;
}

/** Gives the recovery key to `pending` owners; the accounts the chain does not vouch for yet, or what went wrong. */
export async function giveRecoveryKey(held: Unlocked, vouched: Vouched, pending: readonly PendingKey[]): Promise<{ readonly failed: Failure | null; readonly unvouched: readonly string[] }> {
  const path = companyPath(held);
  const mine = await call("GET", `${path}/recovery-key/mine`, readWrap("key_id"));
  if (!mine.ok) {
    return { failed: mine.status === 404 ? refused("no_owner_wrap") : mine, unvouched: [] };
  }
  let given;
  try {
    given = await wrapsForOwners(held.company, held.keys, vouched, mine.value, pending);
  } catch {
    return { failed: refused("recovery_moved"), unvouched: [] };
  }
  if (given.wraps.length === 0) {
    return { failed: null, unvouched: given.unvouched };
  }
  const sent = await call("POST", `${path}/recovery-key/wraps`, ignored, { body: wrapsBody(given) });
  return { failed: sent.ok ? null : sent, unvouched: given.unvouched };
}

/** Hands `request`'s member (whose key set is `memberKey`) their keys; null once handed over, else what went wrong. */
export async function handOverRequest(held: Unlocked, vouched: Vouched, request: OpenRequest, memberKey: DirectoryKey): Promise<Failure | null> {
  const recovery = vouched.recoveryKey();
  const recoveryKeyId = recovery === undefined ? null : keyIdBytes(recovery.keyId);
  const memberKeyId = keyIdBytes(memberKey.keyId);
  const requestId = keyIdBytes(request.id);
  if (recovery === undefined || recoveryKeyId === null || memberKeyId === null || requestId === null) {
    return refused("recovery_moved");
  }
  const path = companyPath(held);
  const mine = await call("GET", `${path}/recovery-key/mine`, readWrap("key_id"));
  if (!mine.ok) {
    return mine.status === 404 ? refused("no_owner_wrap") : mine;
  }
  const escrow = await call("GET", `${path}/keys/${encodeURIComponent(request.account)}/escrow`, readWrap("recovery_key_id"));
  if (!escrow.ok) {
    return escrow.status === 404 ? refused("no_escrow") : escrow;
  }
  if (mine.value.keyId !== recovery.keyId || escrow.value.keyId !== recovery.keyId) {
    return refused("recovery_moved");
  }
  let wrap;
  try {
    wrap = await handOver({
      company: held.company,
      ownerPrivate: held.keys.agreementPrivate,
      ownerPublic: held.keys.agreementPublic,
      ownerKeyId: held.keys.keyId,
      recoveryKeyId,
      recoveryPublic: recovery.agreementPublic,
      recoveryWrap: mine.value.wrap,
      memberKeyId,
      escrow: escrow.value.wrap,
      requestId,
      handoffPublic: request.handoffPublic,
    });
  } catch {
    return refused("unopenable");
  }
  const handed = await call("POST", `${path}/recovery-requests/${encodeURIComponent(request.id)}/handoff`, ignored, { body: handoffBody(wrap) });
  return handed.ok ? null : handed;
}
