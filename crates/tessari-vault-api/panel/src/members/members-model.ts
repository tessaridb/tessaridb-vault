// The members page without a DOM: each member with their key's state against the company's chain and its fingerprint,
// and the body that carries a statement to the server (the statements themselves are made in standing.ts).

import { encode } from "../base64url.ts";
import { sameBytes } from "../crypto/bytes.ts";
import { keyFingerprint } from "../crypto/fingerprint.ts";
import type { Statement, Vouched } from "../crypto/statements.ts";
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
  /** Suspended members stay listed, marked, and cannot sign in until resumed. */
  readonly suspended: boolean;
  /** The listed key's fingerprint, null with no key. */
  readonly fingerprint: string | null;
}

/** Every one of `members`, each with their listed key's state against `vouched`. */
export async function memberRows(members: readonly ListedMember[], directory: ReadonlyMap<string, DirectoryKey>, vouched: Vouched): Promise<readonly MemberRow[]> {
  return Promise.all(
    members.map(async (member): Promise<MemberRow> => {
      const listed = directory.get(member.account);
      const base = { account: member.account, label: member.name, detail: member.email, role: member.role, suspended: !member.active };
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

/** The body `POST /api/companies/{company}/statements` takes for a statement after the genesis. */
export const statementBody = (seq: number, statement: Statement): Readonly<Record<string, unknown>> => ({
  seq,
  bytes: encode(statement.bytes),
  signature: encode(statement.signature),
});
