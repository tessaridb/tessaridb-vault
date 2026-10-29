// Writes chain-vector.json from the browser's own code: `node make-chain-vector.ts > chain-vector.json` in this directory.
import { appendStatement, type Signer, type Statement } from "../../../tessari-vault-api/panel/src/crypto/statements.ts";

async function signer(): Promise<Signer> {
  const pair = (await crypto.subtle.generateKey("Ed25519", true, ["sign", "verify"])) as CryptoKeyPair;
  return { privateKey: pair.privateKey, publicBytes: new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey)) };
}
const x = (n: number) => new Uint8Array(32).fill(n);
const hex = (b: Uint8Array) => Array.from(b, (v) => v.toString(16).padStart(2, "0")).join("");
const company = "c0ffee00c0ffee00c0ffee00c0ffee00";
const owner = await signer();
const admin = await signer();
const stranger = await signer();
const chain: Statement[] = [];
const add = async (fields: Parameters<typeof appendStatement>[2], by: Signer, into = chain) => {
  const s = await appendStatement(company, into.at(-1), fields, by);
  into.push(s);
  return s;
};
await add({ kind: "member", subject: "owner-account", agreementPublic: x(1), signingPublic: owner.publicBytes }, owner);
await add({ kind: "member", subject: "admin-account", agreementPublic: x(2), signingPublic: admin.publicBytes }, owner);
await add({ kind: "admin", subject: "admin-account", agreementPublic: new Uint8Array(0), signingPublic: new Uint8Array(0) }, owner);
const prefix = chain.slice();
const alt = async (fields: Parameters<typeof appendStatement>[2], by: Signer) => (await add(fields, by, prefix.slice()));
const recoveryByAdmin = await alt({ kind: "recovery", subject: "aa".repeat(16), agreementPublic: x(5), signingPublic: new Uint8Array(0) }, admin);
const revokeOwner = await alt({ kind: "revoke", subject: "owner-account", agreementPublic: new Uint8Array(0), signingPublic: new Uint8Array(0) }, admin);
const twoAfter = async (steps: [Parameters<typeof appendStatement>[2], Signer][], base = prefix) => {
  const into = base.slice();
  for (const [fields, by] of steps) await add(fields, by, into);
  return into.slice(base.length);
};
const newAdmin = await signer();
const none = new Uint8Array(0);
const rekeyThenOldAdmin = await twoAfter([
  [{ kind: "member", subject: "admin-account", agreementPublic: x(9), signingPublic: newAdmin.publicBytes }, owner],
  [{ kind: "group", subject: "group-9", agreementPublic: x(9), signingPublic: none }, admin],
]);
const revokedAdmin = await twoAfter([
  [{ kind: "revoke", subject: "admin-account", agreementPublic: none, signingPublic: none }, owner],
  [{ kind: "group", subject: "group-9", agreementPublic: x(9), signingPublic: none }, admin],
]);
// A fork: another third statement, and a fourth on top of it whose prev is not the real third's digest.
const fork = await twoAfter([
  [{ kind: "group", subject: "group-f", agreementPublic: x(10), signingPublic: none }, owner],
  [{ kind: "group", subject: "group-g", agreementPublic: x(11), signingPublic: none }, owner],
], prefix.slice(0, 2));
const byStranger = await alt({ kind: "group", subject: "group-1", agreementPublic: x(6), signingPublic: new Uint8Array(0) }, stranger);
await add({ kind: "group", subject: "group-1", agreementPublic: x(3), signingPublic: new Uint8Array(0) }, admin);
await add({ kind: "recovery", subject: "bb".repeat(16), agreementPublic: x(4), signingPublic: new Uint8Array(0) }, owner);
await add({ kind: "revoke", subject: "group-1", agreementPublic: new Uint8Array(0), signingPublic: new Uint8Array(0) }, admin);
await add({ kind: "member", subject: "member-account", agreementPublic: x(7), signingPublic: x(8) }, admin);
// Signed by the owner key, but naming another key as the owner's own signing key.
const genesisNamingAnotherKey = await appendStatement(company, undefined,
  { kind: "member", subject: "owner-account", agreementPublic: x(1), signingPublic: stranger.publicBytes }, owner);
const out = (s: Statement) => ({ bytes: hex(s.bytes), signature: hex(s.signature) });
console.log(JSON.stringify({
  genesis_naming_another_key: out(genesisNamingAnotherKey),
  company, owner_signing: hex(owner.publicBytes), chain: chain.map(out),
  after_three: { recovery_by_admin: [out(recoveryByAdmin)], revoke_owner: [out(revokeOwner)], by_stranger: [out(byStranger)],
    rekey_then_old_admin: rekeyThenOldAdmin.map(out), revoked_admin: revokedAdmin.map(out), fork_fourth: [out(fork[1]!)] },
}, null, 1));
