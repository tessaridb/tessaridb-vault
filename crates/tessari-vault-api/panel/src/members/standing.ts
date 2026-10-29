// Keeping the chain in step with the company: an admin, or an owner who does not hold the owner key, needs
// admin standing in the chain to vouch from their browser; a member who has it no longer needs it; and a key the
// chain still vouches for after its holder left the company is ended where it stands, keeping what they signed before.
// Every statement is checked against the whole chain with it before it is sent.

import { sameBytes, type Bytes } from "../crypto/bytes.ts";
import { appendStatement, verifyChain, type Signer, type Statement, type StatementFields, type Vouched } from "../crypto/statements.ts";
import type { ListedMember } from "../records/share-targets.ts";
import type { DirectoryKey } from "../records/shares-model.ts";

/** Whether the chain's admin standing for a person matches their role. */
export type Standing = "matches" | "needs-admin" | "needs-demotion";

const NONE = new Uint8Array(0);

/** Whether `role` needs admin standing for the person whose key `signing` is, the owner key needing none. */
function wantsAdmin(role: ListedMember["role"], signing: Bytes | undefined, ownerSigning: Bytes): boolean {
  return role === "admin" || (role === "owner" && (signing === undefined || !sameBytes(signing, ownerSigning)));
}

/** How the chain's standing for `account` compares with their `role`. */
export function standingOf(role: ListedMember["role"], account: string, vouched: Vouched, ownerSigning: Bytes): Standing {
  const wanted = wantsAdmin(role, vouched.signingKeyOf(account), ownerSigning);
  const held = vouched.hasAdminStanding(account);
  return wanted && !held ? "needs-admin" : !wanted && held ? "needs-demotion" : "matches";
}

const memberFields = (account: string, key: DirectoryKey): StatementFields => ({ kind: "member", subject: account, agreementPublic: key.agreementPublic, signingPublic: key.signingPublic });
const bareFields = (kind: "admin" | "leave", account: string): StatementFields => ({ kind, subject: account, agreementPublic: NONE, signingPublic: NONE });

/** `fields` signed by `signer` one after another on `chain`, refused with the chain's reason unless the whole verifies. */
async function appended(company: string, ownerSigning: Bytes, chain: readonly Statement[], signer: Signer, fields: readonly StatementFields[]): Promise<readonly Statement[]> {
  const added: Statement[] = [];
  for (const one of fields) {
    added.push(await appendStatement(company, added.at(-1) ?? chain.at(-1), one, signer));
  }
  await verifyChain(company, ownerSigning, [...chain, ...added]);
  return added;
}

/** The statements vouching for `account`'s listed `key`, with the admin standing their `role` needs. */
export function vouchStatements(company: string, ownerSigning: Bytes, chain: readonly Statement[], signer: Signer, account: string, role: ListedMember["role"], key: DirectoryKey): Promise<readonly Statement[]> {
  const fields = [memberFields(account, key), ...(wantsAdmin(role, key.signingPublic, ownerSigning) ? [bareFields("admin", account)] : [])];
  return appended(company, ownerSigning, chain, signer, fields);
}

/** The statement bringing the chain's standing for `account`, whose vouched key is `key`, in line with their role. */
export function standingStatements(company: string, ownerSigning: Bytes, chain: readonly Statement[], signer: Signer, account: string, key: DirectoryKey, standing: Exclude<Standing, "matches">): Promise<readonly Statement[]> {
  // A fresh member statement for the same key drops the standing and keeps everything the key signed.
  const fields = standing === "needs-admin" ? bareFields("admin", account) : memberFields(account, key);
  return appended(company, ownerSigning, chain, signer, [fields]);
}

/** Accounts the chain vouches a key for that the company no longer lists, never the owner key's holder. */
export function departedAccounts(members: readonly ListedMember[], vouched: Vouched, ownerSigning: Bytes): readonly string[] {
  const listed = new Set(members.map((member) => member.account));
  return [...vouched.accounts()].filter((account) => !listed.has(account) && !sameBytes(vouched.signingKeyOf(account) ?? NONE, ownerSigning));
}

/** The statement ending `account`'s key where it stands. */
export function leaveStatement(company: string, ownerSigning: Bytes, chain: readonly Statement[], signer: Signer, account: string): Promise<readonly Statement[]> {
  return appended(company, ownerSigning, chain, signer, [bareFields("leave", account)]);
}
