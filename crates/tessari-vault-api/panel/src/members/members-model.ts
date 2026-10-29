// The members page without a DOM: each member with their key's state against the company's chain and its fingerprint,
// and the statement an owner or admin appends to vouch for a member's key, checked against the whole chain first.
// The server keeps statements without reading them, so one the chain would refuse would break it for everyone.

import { encode } from "../base64url.ts";
import { sameBytes, type Bytes } from "../crypto/bytes.ts";
import { keyFingerprint } from "../crypto/fingerprint.ts";
import { appendStatement, verifyChain, type Signer, type Statement, type Vouched } from "../crypto/statements.ts";
import type { ListedMember } from "../records/share-targets.ts";
import type { DirectoryKey } from "../records/shares-model.ts";

/** Whether a member's listed key is the one the chain vouches for, is waiting for someone to vouch, or does not exist. */
export type KeyState = "vouched" | "waiting" | "no-key";

/** One row of the members page. */
export interface MemberRow {
  readonly account: string;
  readonly label: string;
  readonly detail: string;
  readonly role: ListedMember["role"];
  readonly state: KeyState;
  /** The listed key's fingerprint, null with no key. */
  readonly fingerprint: string | null;
}

/** The active `members`, each with their listed key's state against `vouched`. */
export async function memberRows(members: readonly ListedMember[], directory: ReadonlyMap<string, DirectoryKey>, vouched: Vouched): Promise<readonly MemberRow[]> {
  return Promise.all(
    members
      .filter((member) => member.active)
      .map(async (member): Promise<MemberRow> => {
        const listed = directory.get(member.account);
        const base = { account: member.account, label: member.name, detail: member.email, role: member.role };
        if (listed === undefined) {
          return { ...base, state: "no-key", fingerprint: null };
        }
        const agreement = vouched.agreementKeyOf(member.account);
        const signing = vouched.signingKeyOf(member.account);
        const matches = agreement !== undefined && signing !== undefined && sameBytes(agreement, listed.agreementPublic) && sameBytes(signing, listed.signingPublic);
        return { ...base, state: matches ? "vouched" : "waiting", fingerprint: await keyFingerprint(listed.agreementPublic, listed.signingPublic) };
      }),
  );
}

/**
 * The statement vouching for `account`'s listed `key`, signed by `signer` after the last of `chain`; refused with the
 * chain's own reason when the whole chain with it would not verify from the owner key `ownerSigning`.
 */
export async function vouchStatement(company: string, ownerSigning: Bytes, chain: readonly Statement[], signer: Signer, account: string, key: DirectoryKey): Promise<Statement> {
  const statement = await appendStatement(company, chain.at(-1), { kind: "member", subject: account, agreementPublic: key.agreementPublic, signingPublic: key.signingPublic }, signer);
  await verifyChain(company, ownerSigning, [...chain, statement]);
  return statement;
}

/** The body `POST /api/companies/{company}/statements` takes for a statement after the genesis. */
export const statementBody = (seq: number, statement: Statement): Readonly<Record<string, unknown>> => ({
  seq,
  bytes: encode(statement.bytes),
  signature: encode(statement.signature),
});
