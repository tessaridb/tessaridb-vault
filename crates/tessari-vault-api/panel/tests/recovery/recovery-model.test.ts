// Recovery without a DOM: what the recovery routes answer, the bodies the browser sends, when a member's keys are
// escrowed again, which notices are new, and the words for each refusal.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { encode } from "../../src/base64url.ts";
import type { StoredKeySet } from "../../src/crypto/key-set.ts";
import {
  escrowWanted,
  newNotices,
  noticeWords,
  readHandoff,
  readNotices,
  readOwnEscrow,
  readRecoveryKey,
  readRequests,
  recoveryFailure,
  rewrapBody,
} from "../../src/recovery/recovery-model.ts";

const ID = "0123456789abcdef0123456789abcdef";
const OTHER = "fedcba9876543210fedcba9876543210";
const bytes = (length: number, fill: number) => new Uint8Array(length).fill(fill);

describe("recovery answers", () => {
  it("reads which recovery key the caller's escrow is wrapped to, and refuses another shape", () => {
    assert.equal(readOwnEscrow({ recovery_key_id: ID }), ID);
    assert.equal(readOwnEscrow({ recovery_key_id: "not hex" }), null);
    assert.equal(readOwnEscrow({}), null);
  });

  it("reads the recovery key with whether it is due for rotation, and refuses a key without that answer", () => {
    const listed = { key_id: ID, public: encode(bytes(32, 1)) };
    assert.deepEqual(readRecoveryKey({ ...listed, rotation_due: true }), { keyId: ID, public: bytes(32, 1), rotationDue: true });
    assert.equal(readRecoveryKey({ ...listed, rotation_due: false })?.rotationDue, false);
    assert.equal(readRecoveryKey(listed), null);
    assert.equal(readRecoveryKey({ ...listed, rotation_due: "yes" }), null);
  });

  it("reads the open requests, each with a 32-byte handoff key", () => {
    const listed = readRequests({ requests: [{ id: ID, account: "acc-1", handoff_public: encode(bytes(32, 7)), created: 1_700_000_000 }] });
    assert.deepEqual(listed, [{ id: ID, account: "acc-1", handoffPublic: bytes(32, 7), created: 1_700_000_000 }]);
    assert.equal(readRequests({ requests: [{ id: ID, account: "acc-1", handoff_public: encode(bytes(31, 7)), created: 1 }] }), null);
    assert.equal(readRequests({ requests: [{ id: "x", account: "acc-1", handoff_public: encode(bytes(32, 7)), created: 1 }] }), null);
  });

  it("reads a handoff wrap and the notices about the caller", () => {
    assert.deepEqual(readHandoff({ wrap: encode(bytes(90, 1)) }), bytes(90, 1));
    assert.equal(readHandoff({ wrap: "" }), null);
    assert.deepEqual(readNotices({ notices: [{ by: "acc-2", kind: "escrow.read", at: 5 }] }), [{ by: "acc-2", kind: "escrow.read", at: 5 }]);
    assert.equal(readNotices({ notices: [{ by: "acc-2", kind: "escrow.read", at: "5" }] }), null);
  });
});

describe("escrowing again", () => {
  it("escrows when the chain vouches a recovery key the escrow is not wrapped to", () => {
    const vouched = { keyId: ID, agreementPublic: bytes(32, 1) };
    assert.equal(escrowWanted(null, vouched), true);
    assert.equal(escrowWanted(OTHER, vouched), true);
    assert.equal(escrowWanted(ID, vouched), false);
    assert.equal(escrowWanted(null, undefined), false);
  });
});

describe("what the browser sends", () => {
  it("rewraps under the restored key set's new salt and parameters, naming the request", () => {
    const stored: StoredKeySet = {
      keyId: bytes(16, 1),
      agreementPublic: bytes(32, 2),
      signingPublic: bytes(32, 3),
      wrapped: bytes(80, 4),
      salt: bytes(16, 5),
      kdf: { memoryKib: 65_536, passes: 3, lanes: 4 },
    };
    assert.deepEqual(rewrapBody(ID, stored), {
      request: ID,
      kdf: { salt: encode(bytes(16, 5)), memory_kib: 65_536, passes: 3, lanes: 4 },
      wrapped: encode(bytes(80, 4)),
    });
  });
});

describe("notices", () => {
  it("keeps only the ones after what this browser last showed", () => {
    const all = [
      { by: "a", kind: "escrow.read", at: 10 },
      { by: "b", kind: "recovery", at: 20 },
    ];
    assert.deepEqual(newNotices(all, 10), [{ by: "b", kind: "recovery", at: 20 }]);
    assert.deepEqual(newNotices(all, null), all);
    assert.deepEqual(newNotices(all, 20), []);
  });
});

describe("notice words", () => {
  it("names who did what, and keeps an unknown kind as written", () => {
    assert.equal(noticeWords("escrow.read", "Kim"), "Kim read your keys kept for recovery");
    assert.equal(noticeWords("recovery", "Kim"), "Kim handed your keys back to you");
    assert.equal(noticeWords("something.new", "Kim"), "Kim: something.new");
  });
});

describe("refusals in words", () => {
  it("says what to do for each recovery refusal, and something for one it does not know", () => {
    const codes = ["no_escrow", "not_handed_over", "recovery_closed", "recovery_keyed", "forbidden", "no_owner_wrap", "recovery_moved", "unopenable", "unvouched-signer", "outside_reach", "network", "something_new"];
    const words = codes.map((code) => recoveryFailure({ ok: false, status: 409, code, message: "" }));
    assert.equal(new Set(words).size, codes.length);
    assert.ok(words.every((each) => each.length > 0 && !/[a-z]_[a-z]/.test(each)));
  });
});
