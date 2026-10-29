// Passkeys without a DOM: the options the server gives, the request the browser makes from them, what a refused
// registration says, and the person's own passkeys as the server lists them.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { creationOptions, passkeyFailure, readPasskeys, readRegistrationOptions } from "../../src/account/passkey.ts";
import { encode } from "../../src/base64url.ts";

const failure = (status: number, code: string) => ({ ok: false as const, status, code, message: "server words" });
const bytes = (...values: number[]) => new Uint8Array(values);

describe("the options a registration starts from", () => {
  it("takes them only in their shape", () => {
    const raw = { challenge: encode(bytes(1, 2, 3)), rp_id: "localhost", user_id: encode(bytes(9, 9)), user_name: "dana@vault.test", exclude: [encode(bytes(7))] };
    const read = readRegistrationOptions(raw);
    assert.ok(read !== null);
    assert.deepEqual([...read.challenge], [1, 2, 3]);
    assert.deepEqual([...read.userId], [9, 9]);
    assert.equal(read.rpId, "localhost");
    assert.equal(read.userName, "dana@vault.test");
    assert.deepEqual(read.exclude.map((each) => [...each]), [[7]]);
    assert.equal(readRegistrationOptions({ ...raw, challenge: "%%" }), null);
    assert.equal(readRegistrationOptions({ ...raw, exclude: "x" }), null);
    assert.equal(readRegistrationOptions({ ...raw, rp_id: 3 }), null);
  });
});

describe("the request the browser makes", () => {
  it("asks for a discoverable, verified passkey in one of the algorithms the vault accepts", () => {
    const read = readRegistrationOptions({ challenge: encode(bytes(1)), rp_id: "localhost", user_id: encode(bytes(2)), user_name: "dana@vault.test", exclude: [encode(bytes(3))] });
    assert.ok(read !== null);
    const made = creationOptions(read, "Dana Owner");
    assert.deepEqual(made.pubKeyCredParams.map((each) => each.alg), [-8, -7, -257]);
    assert.equal(made.rp.id, "localhost");
    assert.equal(made.user.name, "dana@vault.test");
    assert.equal(made.user.displayName, "Dana Owner");
    assert.equal(made.authenticatorSelection?.residentKey, "required");
    assert.equal(made.authenticatorSelection?.userVerification, "required");
    assert.deepEqual(made.excludeCredentials?.map((each) => [...new Uint8Array(each.id as ArrayBuffer)]), [[3]]);
  });
});

describe("what a refused registration says", () => {
  it("names a passkey already registered, a cancelled prompt and a refused one", () => {
    assert.match(passkeyFailure(failure(409, "passkey_registered")), /already/);
    assert.match(passkeyFailure(failure(0, "cancelled")), /cancelled/);
    assert.match(passkeyFailure(failure(400, "passkey_refused")), /wasn't accepted/);
    assert.match(passkeyFailure(failure(0, "network")), /didn't answer/);
  });
});

describe("the person's own passkeys", () => {
  it("are read only in their shape", () => {
    const listed = { passkeys: [{ credential: encode(bytes(7)), created: 1_793_000_000, backup_eligible: true }] };
    assert.deepEqual(readPasskeys(listed), [{ credential: encode(bytes(7)), created: 1_793_000_000, backupEligible: true }]);
    assert.deepEqual(readPasskeys({ passkeys: [] }), []);
    for (const wrong of [{}, { passkeys: [{ credential: "", created: 1, backup_eligible: false }] }, { passkeys: [{ credential: encode(bytes(7)), created: 1.5, backup_eligible: false }] }, { passkeys: [{ credential: encode(bytes(7)), created: 1 }] }]) {
      assert.equal(readPasskeys(wrong), null, JSON.stringify(wrong));
    }
  });

  it("say what a removal that did not happen means", () => {
    assert.match(passkeyFailure(failure(404, "not_found")), /already removed/);
  });
});
