// The company recovery key (C7), made once in the browser that holds the company owner key: a new X25519 pair whose
// public half a `recovery` statement vouches for (the chain refuses one any other key signs, because escrow to it
// reaches every member's keys), and whose private half is wrapped to each owner the chain vouches for, then wiped. An
// owner the chain does not vouch for yet gets no wrap and cannot recover members.

import { hex, random, wipe, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { generatePair, publicBytes } from "../crypto/keys.ts";
import { wrapToVouched } from "../crypto/recipients.ts";
import { appendStatement, verifyChain, type Signer, type Statement, type Vouched } from "../crypto/statements.ts";

const KEY_ID_BYTES = 16;
const KEY_BYTES = 32;

/** An owner's listed key set: whom the recovery key is wrapped to. */
export interface OwnerKey {
  readonly account: string;
  readonly keyId: Bytes;
  readonly agreementPublic: Bytes;
}

/** What making the recovery key sends: the statement vouching for it, then the key with its owners' wraps. */
export interface NewRecoveryKey {
  readonly statement: Statement;
  readonly keyId: Bytes;
  readonly public: Bytes;
  readonly wraps: readonly { readonly keyId: Bytes; readonly wrap: Bytes }[];
  /** Owners left without a wrap because the chain does not vouch for the key the server listed. */
  readonly unvouched: readonly string[];
}

/** A new recovery key vouched by `signer` (the owner key) after `chain`, wrapped to each of `owners` `vouched` names. */
export async function newRecoveryKey(
  company: string,
  signer: Signer,
  ownerSigning: Bytes,
  chain: readonly Statement[],
  vouched: Vouched,
  owners: readonly OwnerKey[],
): Promise<NewRecoveryKey> {
  const pair = await generatePair("X25519");
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
  const raw = pkcs8.slice(pkcs8.length - KEY_BYTES);
  wipe(pkcs8);
  try {
    const keyId = random(KEY_ID_BYTES);
    const recoveryPublic = await publicBytes(pair.publicKey);
    const statement = await appendStatement(company, chain.at(-1), { kind: "recovery", subject: hex(keyId), agreementPublic: recoveryPublic, signingPublic: new Uint8Array(0) }, signer);
    await verifyChain(company, ownerSigning, [...chain, statement]);
    const wraps: { keyId: Bytes; wrap: Bytes }[] = [];
    const unvouched: string[] = [];
    for (const owner of owners) {
      const context = { company, wrappedKind: "recovery-to-owner", wrappedKeyId: keyId, recipientKind: "member", recipientKeyId: owner.keyId } as const;
      try {
        wraps.push({ keyId: owner.keyId, wrap: await wrapToVouched(vouched, { kind: "member", account: owner.account }, owner.agreementPublic, context, raw) });
      } catch (problem) {
        if (!(problem instanceof VaultCryptoError && problem.reason === "unvouched-recipient")) {
          throw problem;
        }
        unvouched.push(owner.account);
      }
    }
    return { statement, keyId, public: recoveryPublic, wraps, unvouched };
  } finally {
    wipe(raw);
  }
}
