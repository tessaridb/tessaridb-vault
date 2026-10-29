// Who a revision of a shared record wraps its new key to: the author's key unless the reviser is the author, and every
// share's key but the reviser's own, each with the public key the server lists for it; a listed key the directory does
// not hold under that id is no recipient at all.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { encode } from "../../src/base64url.ts";
import { hex, type Bytes } from "../../src/crypto/bytes.ts";
import { readDirectory, readShares, revisionRecipients } from "../../src/records/shares-model.ts";

const [AUTHOR, ADMIN, OWNER, TEAM] = ["a".repeat(32), "b".repeat(32), "c".repeat(32), "d".repeat(32)];
const [AUTHOR_KEY, ADMIN_KEY, OWNER_KEY, TEAM_KEY] = ["22".repeat(16), "55".repeat(16), "44".repeat(16), "66".repeat(16)];
const publicKey = (seed: number): Bytes => new Uint8Array(32).fill(seed);

const SHARES = readShares({
  author: { account: AUTHOR, key_id: AUTHOR_KEY },
  shares: [
    { kind: "member", target: ADMIN, right: "edit", key_id: ADMIN_KEY },
    { kind: "group", target: TEAM, right: "read", key_id: TEAM_KEY },
  ],
});
const DIRECTORY = readDirectory({
  keys: [
    { account: AUTHOR, key_id: AUTHOR_KEY, agreement_public: encode(publicKey(1)), signing_public: encode(publicKey(9)) },
    { account: ADMIN, key_id: ADMIN_KEY, agreement_public: encode(publicKey(2)), signing_public: encode(publicKey(9)) },
    { account: OWNER, key_id: OWNER_KEY, agreement_public: encode(publicKey(3)), signing_public: encode(publicKey(9)) },
  ],
});
const GROUPS = new Map([[TEAM, publicKey(4)]]);

const named = (recipients: ReturnType<typeof revisionRecipients>): string[] =>
  (recipients ?? []).map((each) => `${each.recipient.kind}:${hex(each.keyId)}:${each.listed[0]}`).sort();

describe("a shared record's revision recipients", () => {
  it("reads the share list and the key directory", () => {
    assert.ok(SHARES !== null && DIRECTORY !== null);
    assert.equal(readShares({ author: { account: AUTHOR, key_id: null }, shares: [] })?.author.keyId, null);
    assert.equal(readShares({ author: { account: AUTHOR }, shares: [{ kind: "team", target: TEAM, right: "read", key_id: TEAM_KEY }] }), null);
  });

  it("wraps to the author and every share but the reviser's own key", () => {
    assert.ok(SHARES !== null && DIRECTORY !== null);
    assert.deepEqual(named(revisionRecipients(SHARES, DIRECTORY, GROUPS, ADMIN_KEY)), [`group:${TEAM_KEY}:4`, `member:${AUTHOR_KEY}:1`]);
    assert.deepEqual(named(revisionRecipients(SHARES, DIRECTORY, GROUPS, AUTHOR_KEY)), [`group:${TEAM_KEY}:4`, `member:${ADMIN_KEY}:2`]);
  });

  it("gives no recipients when a listed key is not the one the directory holds", () => {
    assert.ok(SHARES !== null && DIRECTORY !== null);
    const moved = readDirectory({
      keys: [{ account: ADMIN, key_id: OWNER_KEY, agreement_public: encode(publicKey(2)), signing_public: encode(publicKey(9)) }],
    });
    assert.ok(moved !== null);
    assert.equal(revisionRecipients(SHARES, moved, GROUPS, AUTHOR_KEY), null);
    assert.equal(revisionRecipients(SHARES, DIRECTORY, new Map(), AUTHOR_KEY), null);
  });
});
