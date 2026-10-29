// Joining with an enrolment link, without a DOM: the authenticator setup the page reads, how its key is shown, the
// password rule checked before anything is sent, and the words each refusal becomes.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { groupedKey, joinFailure, passwordProblem, readAuthenticator, tokenFrom } from "../../src/join-model.ts";

const failure = (status: number, code: string) => ({ ok: false as const, status, code, message: "server words" });

describe("the authenticator setup a join reads", () => {
  it("takes a key and a time-based otpauth link", () => {
    const uri = "otpauth://totp/TessariDB%20Vault:sam%40vault.test?secret=JBSWY3DPEHPK3PXP&issuer=TessariDB%20Vault";
    assert.deepEqual(readAuthenticator({ secret: "JBSWY3DPEHPK3PXP", uri }), { secret: "JBSWY3DPEHPK3PXP", uri });
  });

  it("refuses any other link, so the page never draws a link the server chose freely", () => {
    for (const uri of ["javascript:alert(1)", "https://vault.test/", "otpauth://hotp/x?secret=A", ""]) {
      assert.equal(readAuthenticator({ secret: "JBSWY3DPEHPK3PXP", uri }), null, uri);
    }
    assert.equal(readAuthenticator({ secret: "", uri: "otpauth://totp/x?secret=A" }), null);
    assert.equal(readAuthenticator({ uri: "otpauth://totp/x?secret=A" }), null);
  });
});

describe("the setup key as the page shows it", () => {
  it("groups it by four so it can be typed into an app by hand", () => {
    assert.equal(groupedKey("JBSWY3DPEHPK3PXP"), "JBSW Y3DP EHPK 3PXP");
    assert.equal(groupedKey("ABCDEF"), "ABCD EF");
  });
});

describe("the new sign-in password, before it is sent", () => {
  it("asks for 8 to 256 characters, typed the same twice", () => {
    assert.equal(passwordProblem("long enough", "long enough"), null);
    assert.match(passwordProblem("short", "short") ?? "", /8 to 256/);
    assert.match(passwordProblem("x".repeat(257), "x".repeat(257)) ?? "", /8 to 256/);
    assert.match(passwordProblem("long enough", "long enougH") ?? "", /don't match/);
  });

  it("counts characters, not bytes", () => {
    assert.match(passwordProblem("парольк", "парольк") ?? "", /8 to 256/);
    assert.equal(passwordProblem("п".repeat(256), "п".repeat(256)), null);
  });
});

describe("the token a person pastes", () => {
  it("is the code itself, or what follows token= in a whole link", () => {
    assert.equal(tokenFrom("  abc123  "), "abc123");
    assert.equal(tokenFrom("https://vault.test/#/join?token=abc%2F123"), "abc/123");
    assert.equal(tokenFrom("https://vault.test/#/join?token=abc123&next=%2F"), "abc123");
    // A broken escape is sent as typed, for the server to refuse, rather than failing the page.
    assert.equal(tokenFrom("#/join?token=abc%E0%A4"), "abc%E0%A4");
  });
});

describe("what a refused join says", () => {
  it("names a used or expired link, a wrong code and a refused password", () => {
    assert.match(joinFailure(failure(404, "not_found")), /used already or has expired/);
    assert.match(joinFailure(failure(400, "wrong_code")), /newest code/);
    assert.match(joinFailure(failure(400, "weak_password")), /8 to 256 characters/);
    assert.match(joinFailure(failure(429, "throttled")), /Wait/);
    assert.match(joinFailure(failure(0, "network")), /didn't answer/);
    assert.match(joinFailure(failure(503, "unavailable")), /couldn't/);
  });
});
