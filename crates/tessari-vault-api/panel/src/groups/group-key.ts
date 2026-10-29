// Keying a group in an admin's browser. The group gets a new X25519 pair; a chain statement vouches for its public
// half, so a member uses it only because the owner's chain says so; its private half is wrapped to each member in the
// group's reach whom the chain vouches for, and wiped. A member the chain does not vouch for yet stays pending.

import { encode } from "../base64url.ts";
import { hex, random, sameBytes, wipe, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import type { OpenKeySet } from "../crypto/key-set.ts";
import { generatePair, publicBytes } from "../crypto/keys.ts";
import { wrapToVouched } from "../crypto/recipients.ts";
import { appendStatement, verifyChain, type Signer, type Statement, type Vouched } from "../crypto/statements.ts";
import { unwrapSecret } from "../crypto/wraps.ts";
import type { ListedGroupKey } from "../records/recipient-keys.ts";

const KEY_ID_BYTES = 16;
const KEY_BYTES = 32;

/** A member key in the group's reach with no wrap of the group key yet, as `…/key/pending` lists it. */
export interface PendingKey {
  readonly account: string;
  readonly keyId: Bytes;
  readonly agreementPublic: Bytes;
}

/** What keying a group sends: the statement vouching for it, and the key with its wraps. */
export interface NewGroupKey {
  readonly statement: Statement;
  readonly keyId: Bytes;
  readonly public: Bytes;
  readonly wraps: readonly { readonly keyId: Bytes; readonly wrap: Bytes }[];
  /** Accounts left pending because the chain does not vouch for the key the server listed. */
  readonly unvouched: readonly string[];
}

/** A new key for `group`, vouched by `signer` after `chain`, wrapped to each of `pending` that `vouched` names. */
export async function newGroupKey(
  company: string,
  group: string,
  signer: Signer,
  ownerSigning: Bytes,
  chain: readonly Statement[],
  vouched: Vouched,
  pending: readonly PendingKey[],
): Promise<NewGroupKey> {
  const pair = await generatePair("X25519");
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
  const raw = pkcs8.slice(pkcs8.length - KEY_BYTES);
  wipe(pkcs8);
  try {
    const groupPublic = await publicBytes(pair.publicKey);
    const statement = await appendStatement(company, chain.at(-1), { kind: "group", subject: group, agreementPublic: groupPublic, signingPublic: new Uint8Array(0) }, signer);
    await verifyChain(company, ownerSigning, [...chain, statement]);
    const keyId = random(KEY_ID_BYTES);
    return { statement, keyId, public: groupPublic, ...(await wrapToPending(company, keyId, raw, vouched, pending)) };
  } finally {
    wipe(raw);
  }
}

/** Wraps for members waiting for a group's key: what a holder of the key adds for them. */
export interface PendingWraps {
  readonly wraps: readonly { readonly keyId: Bytes; readonly wrap: Bytes }[];
  readonly unvouched: readonly string[];
}

/**
 * The key of `group`, opened from `member`'s own wrap of it in `listed`, wrapped to each of `pending` the chain vouches
 * for; refused unless the chain vouches for the key the server lists.
 */
export async function pendingWraps(
  company: string,
  group: string,
  member: OpenKeySet,
  vouched: Vouched,
  listed: ListedGroupKey,
  pending: readonly PendingKey[],
): Promise<PendingWraps> {
  const vouchedPublic = vouched.groupKeyOf(group);
  if (vouchedPublic === undefined || !sameBytes(vouchedPublic, listed.public)) {
    throw new VaultCryptoError("unvouched-recipient");
  }
  if (listed.wrap === null) {
    throw new VaultCryptoError("unknown-key");
  }
  const context = { company, wrappedKind: "group", wrappedKeyId: listed.keyId, recipientKind: "member", recipientKeyId: member.keyId } as const;
  const raw = await unwrapSecret(member.agreementPrivate, member.agreementPublic, context, listed.wrap);
  try {
    return await wrapToPending(company, listed.keyId, raw, vouched, pending);
  } finally {
    wipe(raw);
  }
}

async function wrapToPending(company: string, keyId: Bytes, raw: Bytes, vouched: Vouched, pending: readonly PendingKey[]): Promise<PendingWraps> {
  const wraps: { keyId: Bytes; wrap: Bytes }[] = [];
  const unvouched: string[] = [];
  for (const member of pending) {
    const context = { company, wrappedKind: "group", wrappedKeyId: keyId, recipientKind: "member", recipientKeyId: member.keyId } as const;
    try {
      wraps.push({ keyId: member.keyId, wrap: await wrapToVouched(vouched, { kind: "member", account: member.account }, member.agreementPublic, context, raw) });
    } catch (problem) {
      if (!(problem instanceof VaultCryptoError && problem.reason === "unvouched-recipient")) {
        throw problem;
      }
      unvouched.push(member.account);
    }
  }
  return { wraps, unvouched };
}

/** The body `POST …/groups/{group}/key/wraps` takes for `given`. */
export const wrapsBody = (given: PendingWraps): Readonly<Record<string, unknown>> => ({
  wraps: given.wraps.map((each) => ({ key_id: hex(each.keyId), wrap: encode(each.wrap) })),
});

/** The body `PUT /api/companies/{company}/groups/{group}/key` takes for `made`. */
export const groupKeyBody = (made: NewGroupKey): Readonly<Record<string, unknown>> => ({
  key_id: hex(made.keyId),
  public: encode(made.public),
  wraps: made.wraps.map((each) => ({ key_id: hex(each.keyId), wrap: encode(each.wrap) })),
});
