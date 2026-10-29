// What the records views fetch and open: the company's chain, verified from the owner key the person's key set pinned,
// and each opened record, opened with the person's own key or a group key fetched the first time a wrap needs it.

import { call, type Failure } from "../api.ts";
import { readChain } from "../chain-model.ts";
import { hex } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { verifyChain, type Vouched } from "../crypto/statements.ts";
import { readGroupIds, readGroupKey, type OpenedRecord } from "../records-model.ts";
import type { Unlocked } from "../vault.ts";
import type { Listed } from "./listing.ts";
import { openRecord, type RecipientKey } from "./open-record.ts";
import { groupKey, memberKey, type ListedGroupKey } from "./recipient-keys.ts";
import { groupsToOpen } from "./audience.ts";

export const companyPath = (held: Unlocked): string => `/api/companies/${encodeURIComponent(held.company)}`;

/** The chain verified from the pinned owner key, a failed request, or the refusal that broke it. */
export async function vouchedChain(held: Unlocked): Promise<{ ok: true; vouched: Vouched } | Failure | { ok: false; problem: VaultCryptoError }> {
  const chain = await call("GET", `${companyPath(held)}/statements`, readChain);
  if (!chain.ok) {
    return chain;
  }
  try {
    return { ok: true, vouched: await verifyChain(held.company, held.keys.ownerSigningPublic, chain.value.statements) };
  } catch (problem) {
    return { ok: false, problem: problem instanceof VaultCryptoError ? problem : new VaultCryptoError("chain-broken") };
  }
}

/** Opens records for one view; the group keys it fetches live as long as the view. */
export class Opener {
  readonly #held: Unlocked;
  readonly #vouched: Vouched;
  #groups: Promise<ReadonlyMap<string, { group: string; listed: ListedGroupKey }>> | null = null;

  constructor(held: Unlocked, vouched: Vouched) {
    this.#held = held;
    this.#vouched = vouched;
  }

  /** The record opened, or the reason it would not open. */
  async open(opened: OpenedRecord): Promise<Listed> {
    try {
      const key = await this.keyFor(opened);
      return { row: opened.row, through: opened.through, document: await openRecord(this.#held.company, this.#vouched, key, opened) };
    } catch (problem) {
      return { row: opened.row, through: opened.through, problem: problem instanceof VaultCryptoError ? problem.reason : "malformed" };
    }
  }

  /** The person's own key or the group key `opened`'s wrap is kept for; a share re-opens that wrap with it. */
  async keyFor(opened: OpenedRecord): Promise<RecipientKey> {
    if (opened.recipient.kind === "member") {
      return memberKey(this.#held.keys);
    }
    this.#groups ??= this.#groupKeys();
    const found = (await this.#groups).get(opened.recipient.keyId);
    if (found === undefined) {
      throw new VaultCryptoError("unknown-key");
    }
    return groupKey(this.#held.company, this.#vouched, this.#held.keys, found.group, found.listed);
  }

  /** Every group key the company lists, by key id; a group the person cannot read is left out. */
  async #groupKeys(): Promise<ReadonlyMap<string, { group: string; listed: ListedGroupKey }>> {
    const ids = await call("GET", `${companyPath(this.#held)}/groups`, readGroupIds);
    const keys = new Map<string, { group: string; listed: ListedGroupKey }>();
    if (!ids.ok) {
      return keys;
    }
    for (const group of groupsToOpen(this.#held.company, ids.value)) {
      const listed = await call("GET", `${companyPath(this.#held)}/groups/${encodeURIComponent(group)}/key`, readGroupKey);
      if (listed.ok) {
        keys.set(hex(listed.value.keyId), { group, listed: listed.value });
      }
    }
    return keys;
  }
}
