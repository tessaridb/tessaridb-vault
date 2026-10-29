// Deleting a company from the Company page: the statement an owner's browser signs, what the page reads back about a
// deletion already asked for, and when the delete button may be pressed.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { decode } from "../../src/base64url.ts";
import { hex } from "../../src/crypto/bytes.ts";
import { generatePair, publicBytes } from "../../src/crypto/keys.ts";
import { verify } from "../../src/crypto/signatures.ts";
import { deletionReady, dropFailure, dropStatement, readStanding, signedDrop } from "../../src/company/company-model.ts";

const COMPANY = "0123456789abcdef0123456789abcdef";
const AT = 1_793_300_000;
// The bytes worked out apart from both implementations: label, version 1, the act, lp(company), u64 at.
const REQUEST_HEX =
  "7465737361726964622d7661756c742f76312f636f6d70616e792d64726f700101000000203031323334353637383961626364656630313233343536373839616263646566000000006ae39620";
const CANCEL_HEX = REQUEST_HEX.replace("64726f700101", "64726f700102");

describe("the deletion statement", () => {
  it("is the bytes the server verifies, for a request and for a cancel", () => {
    assert.equal(hex(dropStatement("request", COMPANY, AT)), REQUEST_HEX);
    assert.equal(hex(dropStatement("cancel", COMPANY, AT)), CANCEL_HEX);
  });

  it("is signed now by the owner's signing key, in the stored signature format", async () => {
    const pair = await generatePair("Ed25519");
    const signed = await signedDrop(pair.privateKey, "request", COMPANY, () => AT * 1000 + 999);
    assert.equal(signed.at, AT, "whole seconds, rounded down");
    const signature = decode(signed.signature);
    assert.ok(signature !== null);
    await verify(await publicBytes(pair.publicKey), dropStatement("request", COMPANY, AT), signature);
    await assert.rejects(verify(await publicBytes(pair.publicKey), dropStatement("cancel", COMPANY, AT), signature));
  });
});

describe("where a deletion stands", () => {
  it("reads a request waiting for the platform admin and a confirmed one with its date", () => {
    assert.deepEqual(readStanding({ state: "requested" }), { state: "requested" });
    assert.deepEqual(readStanding({ state: "scheduled", drop_after: 1_793_904_800 }), { state: "scheduled", dropAfter: 1_793_904_800 });
    assert.equal(readStanding({ state: "scheduled" }), null, "a confirmed deletion always says when");
    assert.equal(readStanding({ state: "gone" }), null);
  });
});

describe("the delete button", () => {
  it("waits for the company's exact name and a reason", () => {
    assert.equal(deletionReady("Alpha Ltd", "moving off the vault", "Alpha Ltd"), true);
    assert.equal(deletionReady(" Alpha Ltd ", "moving", "Alpha Ltd"), true, "spaces around the name are not the name");
    assert.equal(deletionReady("alpha ltd", "moving", "Alpha Ltd"), false, "the name exactly, case included");
    assert.equal(deletionReady("Alpha", "moving", "Alpha Ltd"), false);
    assert.equal(deletionReady("Alpha Ltd", "   ", "Alpha Ltd"), false, "a reason is required");
  });
});

describe("a refused deletion step", () => {
  it("says what happened and what to do next", () => {
    const refused = (code: string) => dropFailure({ ok: false, status: 409, code, message: "" });
    assert.match(refused("drop_unsigned"), /key/);
    assert.match(refused("drop_scheduled"), /already confirmed/);
    assert.match(refused("forbidden"), /Only an owner/);
    assert.match(refused("network"), /didn't answer/);
    assert.notEqual(refused("drop_unsigned"), refused("something_else"));
  });
});
