// Signing in, without a DOM: the answers the page reads, the words each refusal becomes, where a person goes after
// signing in, the guard that sends them there, and the one function every call goes through.

import assert from "node:assert/strict";
import { afterEach, describe, it } from "node:test";

import { call } from "../../src/api.ts";
import { decode, encode } from "../../src/base64url.ts";
import { afterSignIn, needsSession } from "../../src/guards.ts";
import { bearer, forget, keep } from "../../src/session.ts";
import {
  isProviderReturn,
  readChallenge,
  readMethods,
  readPasskeyRequest,
  readSession,
  signInFailure,
} from "../../src/sign-in-model.ts";

const failure = (status: number, code: string) => ({ ok: false as const, status, code, message: "server words" });

describe("the answers signing in reads", () => {
  it("takes the methods only when all three are booleans", () => {
    assert.deepEqual(readMethods({ password: true, passkey: false, provider: true }), {
      password: true,
      passkey: false,
      provider: true,
    });
    assert.equal(readMethods({ password: true, passkey: "yes", provider: true }), null);
    assert.equal(readMethods({ password: true, passkey: false }), null);
    assert.equal(readMethods([true, true, true]), null);
  });

  it("takes a challenge, a session and a passkey request only when present and non-empty", () => {
    assert.equal(readChallenge({ challenge: "c1" }), "c1");
    assert.equal(readChallenge({ challenge: "" }), null);
    assert.equal(readSession({ session: "s1" }), "s1");
    assert.equal(readSession({ token: "s1" }), null);
    assert.deepEqual(readPasskeyRequest({ challenge: "c", rp_id: "vault.test" }), { challenge: "c", rpId: "vault.test" });
    assert.equal(readPasskeyRequest({ challenge: "c" }), null);
  });
});

describe("what a refused sign-in says", () => {
  it("names a distinct next step for every code the sign-in routes answer", () => {
    const codes: [number, string][] = [
      [401, "sign_in_failed"],
      [400, "wrong_code"],
      [400, "passkey_refused"],
      [429, "throttled"],
      [0, "network"],
      [503, "unavailable"],
      [500, "internal"],
    ];
    const words = codes.map(([status, code]) => signInFailure(failure(status, code)));
    assert.equal(new Set(words).size, codes.length, words.join(" | "));
    for (const said of words) {
      assert.doesNotMatch(said, /server words|oops|error \d|!/i);
    }
    assert.match(signInFailure(failure(429, "throttled")), /wait/i);
  });
});

describe("where signing in leads", () => {
  it("returns to a path of this page and nowhere else", () => {
    assert.equal(afterSignIn("/records/r1"), "#/records/r1");
    assert.equal(afterSignIn(null), "#/");
    assert.equal(afterSignIn("//evil.test/x"), "#/");
    assert.equal(afterSignIn("https://evil.test"), "#/");
    assert.equal(afterSignIn("/sign-in?next=/x"), "#/");
  });

  it("sends a person with no session to sign in, carrying where they were going", () => {
    let signedIn = false;
    const guard = needsSession(() => signedIn, () => "#/records/r1?tab=share");
    assert.deepEqual(guard(), { kind: "redirect", to: "#/sign-in?next=%2Frecords%2Fr1%3Ftab%3Dshare" });
    signedIn = true;
    assert.deepEqual(guard(), { kind: "allow" });
  });

  it("recognises an identity provider's return by its state and code", () => {
    assert.equal(isProviderReturn("?code=a&state=b"), true);
    assert.equal(isProviderReturn("?error=access_denied&state=b"), true);
    assert.equal(isProviderReturn("?code=a"), false);
    assert.equal(isProviderReturn(""), false);
  });
});

describe("base64url", () => {
  it("round-trips every length and refuses what is not base64url", () => {
    for (let length = 0; length < 40; length += 1) {
      const bytes = Uint8Array.from({ length }, (_, at) => (at * 37 + 11) % 256);
      const text = encode(bytes);
      assert.match(text, /^[A-Za-z0-9_-]*$/);
      assert.equal(text, Buffer.from(bytes).toString("base64url"));
      assert.deepEqual(decode(text), bytes);
    }
    assert.equal(decode("a+b/"), null);
    assert.equal(decode("abcde"), null);
  });
});

describe("the call every request goes through", () => {
  const realFetch = globalThis.fetch;
  afterEach(() => {
    globalThis.fetch = realFetch;
    forget();
  });

  function answering(status: number, body: unknown, seen: Request[] = []): void {
    globalThis.fetch = async (input, init) => {
      seen.push(new Request(new URL(String(input), "https://vault.test"), init));
      return new Response(body === null ? null : JSON.stringify(body), { status });
    };
  }

  it("carries the session's bearer and a stated reason, and reads the answer", async () => {
    const seen: Request[] = [];
    answering(200, { session: "s2" }, seen);
    keep("token-1");
    const answer = await call("POST", "/api/x", readSession, { body: { a: 1 }, reason: "rotating" });
    assert.deepEqual(answer, { ok: true, value: "s2" });
    assert.equal(seen[0]?.headers.get("authorization"), "Bearer token-1");
    assert.equal(seen[0]?.headers.get("vault-reason"), "rotating");
    assert.equal(seen[0]?.headers.get("content-type"), "application/json");
  });

  it("sends no bearer without a session", async () => {
    const seen: Request[] = [];
    answering(200, { session: "s2" }, seen);
    assert.equal(bearer(), null);
    await call("GET", "/api/x", readSession);
    assert.equal(seen[0]?.headers.get("authorization"), null);
  });

  it("turns a refusal, a wrong shape and a dead network into failures", async () => {
    answering(401, { error_code: "sign_in_failed", message: "no" });
    assert.deepEqual(await call("POST", "/api/sign-in", readChallenge), {
      ok: false,
      status: 401,
      code: "sign_in_failed",
      message: "no",
    });
    answering(200, { nope: true });
    const shaped = await call("GET", "/api/x", readSession);
    assert.equal(shaped.ok ? "" : shaped.code, "bad_answer");
    globalThis.fetch = async () => {
      throw new TypeError("offline");
    };
    const dead = await call("GET", "/api/x", readSession);
    assert.equal(dead.ok ? "" : dead.code, "network");
  });
});
