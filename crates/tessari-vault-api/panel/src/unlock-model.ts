// What unlocking reads and sends besides the page: who is signed in, the companies they belong to, their key set as
// the server stores it, and what each refusal means in words. No DOM here, so all of it is tested.

import { decode, encode } from "./base64url.ts";
import { hex, type Bytes } from "./crypto/bytes.ts";
import type { StoredKeySet } from "./crypto/key-set.ts";

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const isText = (v: unknown): v is string => typeof v === "string" && v !== "";
const isCount = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v > 0;

/** The signed-in person. */
export interface Me {
  readonly account: string;
  readonly name: string;
  readonly email: string;
  readonly platformAdmin: boolean;
}

export function readMe(v: unknown): Me | null {
  if (!isRecord(v) || !isRecord(v["account"]) || typeof v["platform_admin"] !== "boolean") {
    return null;
  }
  const { id, name, email } = v["account"];
  return isText(id) && isText(name) && isText(email) ? { account: id, name, email, platformAdmin: v["platform_admin"] } : null;
}

export type Role = "owner" | "admin" | "member";

/** A company the person can see; `role` is null for a platform admin who is not a member of it. */
export interface Company {
  readonly id: string;
  readonly name: string;
  readonly active: boolean;
  readonly role: Role | null;
}

const isRole = (v: unknown): v is Role => v === "owner" || v === "admin" || v === "member";

export function readCompanies(v: unknown): readonly Company[] | null {
  if (!isRecord(v) || !Array.isArray(v["companies"])) {
    return null;
  }
  const companies: Company[] = [];
  for (const item of v["companies"]) {
    if (!isRecord(item) || !isText(item["id"]) || !isText(item["name"]) || !isText(item["state"])) {
      return null;
    }
    const role = item["role"];
    if (role !== null && !isRole(role)) {
      return null;
    }
    companies.push({ id: item["id"], name: item["name"], active: item["state"] === "active", role });
  }
  return companies;
}

/** The companies whose records this person can unlock: active, and a member of. */
export const unlockable = (companies: readonly Company[]): readonly Company[] =>
  companies.filter((company) => company.active && company.role !== null);

/** A key id: 32 lowercase hexadecimal characters as its 16 bytes. */
function keyIdBytes(text: string): Bytes | null {
  if (!/^[0-9a-f]{32}$/.test(text)) {
    return null;
  }
  return Uint8Array.from(text.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

/** The member's key set as `GET /api/companies/{company}/keys/me` answers it. */
export function readOwnKeys(v: unknown): StoredKeySet | null {
  if (!isRecord(v) || !isRecord(v["kdf"]) || !isText(v["key_id"])) {
    return null;
  }
  const { salt, memory_kib: memoryKib, passes, lanes } = v["kdf"];
  if (!isText(salt) || !isCount(memoryKib) || !isCount(passes) || !isCount(lanes)) {
    return null;
  }
  const keyId = keyIdBytes(v["key_id"]);
  const parts = [v["agreement_public"], v["signing_public"], v["wrapped"], salt].map((part) => (isText(part) ? decode(part) : null));
  const [agreementPublic, signingPublic, wrapped, saltBytes] = parts;
  if (keyId === null || agreementPublic == null || signingPublic == null || wrapped == null || saltBytes == null) {
    return null;
  }
  return { keyId, agreementPublic, signingPublic, wrapped, salt: saltBytes, kdf: { memoryKib, passes, lanes } };
}

/** The body `PUT /api/companies/{company}/keys/me` takes for a new key set. */
export const keySetBody = (stored: StoredKeySet): Readonly<Record<string, unknown>> => ({
  key_id: hex(stored.keyId),
  agreement_public: encode(stored.agreementPublic),
  signing_public: encode(stored.signingPublic),
  wrapped: encode(stored.wrapped),
  kdf: { salt: encode(stored.salt), memory_kib: stored.kdf.memoryKib, passes: stored.kdf.passes, lanes: stored.kdf.lanes },
});

/** The shortest unlock password the page accepts; the server never sees it, so the page is the only place to say so. */
export const UNLOCK_MINIMUM = 12;

/** Why a new unlock password is refused, or null when it may be used. */
export function newPasswordProblem(password: string, repeated: string, signInEmail: string): string | null {
  if ([...password].length < UNLOCK_MINIMUM) {
    return `Use at least ${UNLOCK_MINIMUM} characters. A phrase of a few words is easiest to remember.`;
  }
  if (password !== repeated) {
    return "The two passwords don't match. Type the same one twice.";
  }
  if (password.toLowerCase() === signInEmail.toLowerCase()) {
    return "Don't use your email address as your unlock password.";
  }
  return null;
}

/** The server's answer to keeping a key set: the key id it kept, which must be the one sent. */
export const readKept =
  (sent: Bytes) =>
  (v: unknown): string | null =>
    isRecord(v) && v["key_id"] === hex(sent) ? hex(sent) : null;
