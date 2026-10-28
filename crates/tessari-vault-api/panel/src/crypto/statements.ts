// Layer 3 (KH-L3): the vouching chain. The company owner's signing key OSK, pinned by the company row's fingerprint,
// signs the genesis; every later statement extends the chain by one (seq + 1, prev = SHA-256 of the previous statement
// and its signature) and is signed by OSK or by an admin the chain made an admin and has not revoked. A client wraps
// only to keys this chain vouches for and trusts revisions only from them. Any break refuses the WHOLE chain: a
// verified prefix is not offered, because the server choosing where a chain ends is how it would hide a revocation.
//
// Statement bytes: "tessaridb-vault/v1/statement" ‖ [version] ‖ [kind] ‖ lp(company) ‖ u64 seq ‖ prev 32 ‖ lp(subject) ‖
// lp(agreement public) ‖ lp(signing public) ‖ signer public 32.

import { Reader, concat, field, hex, integer, sameBytes, utf8, type Bytes } from "./bytes.ts";
import { VaultCryptoError } from "./errors.ts";
import { FORMAT_VERSION, LABEL } from "./labels.ts";
import { sign, verify } from "./signatures.ts";

const KEY_BYTES = 32;
const DIGEST_BYTES = 32;
const KINDS = ["member", "admin", "group", "revoke"] as const;

export type StatementKind = (typeof KINDS)[number];

/** What a statement says; the chain position and the signer are added by `appendStatement`. */
export interface StatementFields {
  kind: StatementKind;
  /** An account id (member, admin, revoke) or a group id (group, revoke). */
  subject: string;
  /** The subject's X25519 public key: required for member and group, empty otherwise. */
  agreementPublic: Bytes;
  /** The subject's Ed25519 public key: required for member, empty otherwise. */
  signingPublic: Bytes;
}

export interface Statement {
  bytes: Bytes;
  signature: Bytes;
}

export interface Signer {
  privateKey: CryptoKey;
  publicBytes: Bytes;
}

interface Decoded extends StatementFields {
  company: string;
  seq: number;
  prev: Bytes;
  signerPublic: Bytes;
}

interface MemberKeys {
  agreement: Bytes;
  signing: Bytes;
}

/** The keys a verified chain vouches for, by account or group id. */
export class Vouched {
  readonly #members: ReadonlyMap<string, MemberKeys>;
  readonly #groups: ReadonlyMap<string, Bytes>;

  constructor(members: ReadonlyMap<string, MemberKeys>, groups: ReadonlyMap<string, Bytes>) {
    this.#members = members;
    this.#groups = groups;
  }

  signingKeyOf(account: string): Bytes | undefined {
    return this.#members.get(account)?.signing;
  }

  agreementKeyOf(account: string): Bytes | undefined {
    return this.#members.get(account)?.agreement;
  }

  groupKeyOf(group: string): Bytes | undefined {
    return this.#groups.get(group);
  }
}

function encode(company: string, seq: number, prev: Bytes, fields: StatementFields, signerPublic: Bytes): Bytes {
  return concat(
    utf8(LABEL.statement),
    Uint8Array.of(FORMAT_VERSION, KINDS.indexOf(fields.kind) + 1),
    field(utf8(company)),
    integer(seq, 8),
    prev,
    field(utf8(fields.subject)),
    field(fields.agreementPublic),
    field(fields.signingPublic),
    signerPublic,
  );
}

function decode(bytes: Bytes): Decoded {
  const reader = new Reader(bytes);
  if (!sameBytes(reader.take(utf8(LABEL.statement).length), utf8(LABEL.statement))) {
    throw new VaultCryptoError("malformed");
  }
  if (reader.integer(1) !== FORMAT_VERSION) {
    throw new VaultCryptoError("unknown-version");
  }
  const kind = KINDS[reader.integer(1) - 1];
  if (kind === undefined) {
    throw new VaultCryptoError("chain-broken");
  }
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const text = (raw: Bytes): string => {
    try {
      return decoder.decode(raw);
    } catch {
      throw new VaultCryptoError("malformed");
    }
  };
  const decoded: Decoded = {
    kind,
    company: text(reader.field()),
    seq: reader.integer(8),
    prev: reader.take(DIGEST_BYTES),
    subject: text(reader.field()),
    agreementPublic: reader.field(),
    signingPublic: reader.field(),
    signerPublic: reader.take(KEY_BYTES),
  };
  reader.end();
  return decoded;
}

async function digest(statement: Statement): Promise<Bytes> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", concat(statement.bytes, statement.signature)));
}

/** Extend the chain after `previous` (none: the genesis) with `fields`, signed by `signer`. */
export async function appendStatement(
  company: string,
  previous: Statement | undefined,
  fields: StatementFields,
  signer: Signer,
): Promise<Statement> {
  const seq = previous === undefined ? 1 : decode(previous.bytes).seq + 1;
  const prev = previous === undefined ? new Uint8Array(DIGEST_BYTES) : await digest(previous);
  const bytes = encode(company, seq, prev, fields, signer.publicBytes);
  return { bytes, signature: await sign(signer.privateKey, bytes) };
}

function hasShape(statement: Decoded): boolean {
  const agreement = statement.agreementPublic.length;
  const signing = statement.signingPublic.length;
  switch (statement.kind) {
    case "member":
      return agreement === KEY_BYTES && signing === KEY_BYTES;
    case "group":
      return agreement === KEY_BYTES && signing === 0;
    case "admin":
    case "revoke":
      return agreement === 0 && signing === 0;
  }
}

/** Verify the whole chain against the owner key the company row pins, and answer what it vouches for. */
export async function verifyChain(company: string, ownerSigningPublic: Bytes, chain: readonly Statement[]): Promise<Vouched> {
  if (chain.length === 0) {
    throw new VaultCryptoError("chain-broken");
  }
  const members = new Map<string, MemberKeys>();
  const groups = new Map<string, Bytes>();
  // The signing keys of members made admin and not revoked since, by hex; OSK is checked on its own.
  const adminKeys = new Set<string>();
  const forget = (subject: string): boolean => {
    const keys = members.get(subject);
    if (keys !== undefined) {
      adminKeys.delete(hex(keys.signing));
    }
    return members.delete(subject);
  };
  let expectedPrev: Bytes = new Uint8Array(DIGEST_BYTES);
  let ownerAccount = "";
  for (const [index, statement] of chain.entries()) {
    const decoded = decode(statement.bytes);
    await verify(decoded.signerPublic, statement.bytes, statement.signature);
    const placed = decoded.company === company && decoded.seq === index + 1 && sameBytes(decoded.prev, expectedPrev);
    if (!placed || !hasShape(decoded)) {
      throw new VaultCryptoError("chain-broken");
    }
    const byOwner = sameBytes(decoded.signerPublic, ownerSigningPublic);
    if (index === 0) {
      if (!byOwner || decoded.kind !== "member" || !sameBytes(decoded.signingPublic, ownerSigningPublic)) {
        throw new VaultCryptoError("chain-broken");
      }
      ownerAccount = decoded.subject;
    } else if (!byOwner && !adminKeys.has(hex(decoded.signerPublic))) {
      throw new VaultCryptoError("unvouched-signer");
    }
    switch (decoded.kind) {
      case "member":
        forget(decoded.subject);
        members.set(decoded.subject, { agreement: decoded.agreementPublic, signing: decoded.signingPublic });
        break;
      case "admin": {
        const keys = members.get(decoded.subject);
        if (keys === undefined) {
          throw new VaultCryptoError("chain-broken");
        }
        adminKeys.add(hex(keys.signing));
        break;
      }
      case "group":
        groups.set(decoded.subject, decoded.agreementPublic);
        break;
      case "revoke":
        if (decoded.subject === ownerAccount || (!forget(decoded.subject) && !groups.delete(decoded.subject))) {
          throw new VaultCryptoError("chain-broken");
        }
        break;
    }
    expectedPrev = await digest(statement);
  }
  return new Vouched(members, groups);
}
