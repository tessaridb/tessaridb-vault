// Unlocking, without a DOM: the answers it reads and sends, which companies can be unlocked, which new unlock passwords
// are refused, the guards in front of the records, and the vault that locks by hand and when idle.

import assert from "node:assert/strict";
import { afterEach, describe, it } from "node:test";

import { encode } from "../../src/base64url.ts";
import type { OpenKeySet } from "../../src/crypto/key-set.ts";
import { afterUnlock, every, needsSession, needsUnlock } from "../../src/guards.ts";
import {
  keySetBody,
  firstSave,
  newPasswordProblem,
  readCompanies,
  readKept,
  readMe,
  readOwnKeys,
  unlockable,
} from "../../src/unlock-model.ts";
import { hold, lock, lockWhenIdle, onLock, unlocked } from "../../src/vault.ts";

const bytes = (length: number, fill: number) => new Uint8Array(length).fill(fill);
const KEY_ID = "0123456789abcdef0123456789abcdef";

const wire = {
  key_id: KEY_ID,
  agreement_public: encode(bytes(32, 1)),
  signing_public: encode(bytes(32, 2)),
  wrapped: encode(bytes(80, 3)),
  kdf: { salt: encode(bytes(16, 4)), memory_kib: 65_536, passes: 3, lanes: 4 },
};

describe("what unlocking reads and sends", () => {
  it("reads who is signed in", () => {
    assert.deepEqual(readMe({ account: { id: "a1", email: "e@x.test", name: "Ana" }, platform_admin: false }), {
      account: "a1",
      email: "e@x.test",
      name: "Ana",
      platformAdmin: false,
    });
    assert.equal(readMe({ account: { id: "a1", email: "e@x.test" }, platform_admin: false }), null);
  });

  it("reads companies and offers the open ones the person belongs to, a closing one included", () => {
    const companies = readCompanies({
      companies: [
        { id: "c1", name: "Alpha", state: "active", role: "member" },
        { id: "c2", name: "Beta", state: "suspended", role: "owner" },
        { id: "c3", name: "Gamma", state: "active", role: null },
        { id: "c4", name: "Delta", state: "dropping", role: "member" },
      ],
    });
    assert.ok(companies !== null);
    assert.deepEqual(
      unlockable(companies).map((company) => company.id),
      ["c1", "c4"],
      "a company waiting to be deleted is read-only, not closed: its members still read, its owner may cancel",
    );
    assert.equal(readCompanies({ companies: [{ id: "c1", name: "Alpha", state: "active", role: "boss" }] }), null);
  });

  it("reads a key set and sends one back in the same shape", () => {
    const stored = readOwnKeys(wire);
    assert.ok(stored !== null);
    assert.equal(stored.keyId.length, 16);
    assert.deepEqual(stored.kdf, { memoryKib: 65_536, passes: 3, lanes: 4 });
    assert.deepEqual(keySetBody(stored), wire);
    assert.equal(readOwnKeys({ ...wire, key_id: "XYZ" }), null);
    assert.equal(readOwnKeys({ ...wire, wrapped: "not base64url!" }), null);
    assert.equal(readOwnKeys({ ...wire, kdf: { ...wire.kdf, passes: 0 } }), null);
  });

  it("takes the kept answer only when it names the key set sent", () => {
    const sent = bytes(16, 0xab);
    assert.equal(readKept(sent)({ key_id: "ab".repeat(16) }), "ab".repeat(16));
    assert.equal(readKept(sent)({ key_id: "cd".repeat(16) }), null);
  });
});

describe("a new unlock password", () => {
  it("must be long, typed twice the same, and not the email", () => {
    assert.match(newPasswordProblem("short", "short", "e@x.test") ?? "", /12 characters/);
    assert.match(newPasswordProblem("long enough phrase", "long enough phrasE", "e@x.test") ?? "", /don't match/);
    assert.match(newPasswordProblem("ana@alpha.test", "ana@alpha.test", "Ana@Alpha.test") ?? "", /email/);
    assert.equal(newPasswordProblem("a phrase of my own", "a phrase of my own", "e@x.test"), null);
  });
});

describe("the guards in front of the records", () => {
  it("ask for a session first, then for unlocking, each carrying where the person was going", () => {
    let signedIn = false;
    let open = false;
    const guard = every(needsSession(() => signedIn, () => "#/"), needsUnlock(() => open, () => "#/"));
    assert.deepEqual(guard(), { kind: "redirect", to: "#/sign-in?next=%2F" });
    signedIn = true;
    assert.deepEqual(guard(), { kind: "redirect", to: "#/unlock?next=%2F" });
    open = true;
    assert.deepEqual(guard(), { kind: "allow" });
  });

  it("return after unlocking to a path of this page that is not a gate", () => {
    assert.equal(afterUnlock("/records"), "#/records");
    assert.equal(afterUnlock("/unlock?next=/"), "#/");
    assert.equal(afterUnlock("/sign-in"), "#/");
    assert.equal(afterUnlock("//evil.test"), "#/");
  });
});

describe("the vault", () => {
  // The vault holds the keys and never reads them, so an empty stand-in is enough here.
  const keys = {} as OpenKeySet;
  afterEach(() => lock());

  /** A clock the test advances by hand. */
  function manualClock(): { set: (run: () => void, ms: number) => unknown; clear: (handle: unknown) => void; pass: (ms: number) => void } {
    let now = 0;
    const timers = new Map<number, { at: number; run: () => void }>();
    let next = 1;
    return {
      set: (run, ms) => {
        timers.set(next, { at: now + ms, run });
        return next++;
      },
      clear: (handle) => timers.delete(Number(handle)),
      pass: (ms) => {
        now += ms;
        for (const [id, timer] of [...timers]) {
          if (timer.at <= now) {
            timers.delete(id);
            timer.run();
          }
        }
      },
    };
  }

  it("locks by hand and tells its listeners once", () => {
    let told = 0;
    const stop = onLock(() => {
      told += 1;
    });
    hold({ company: "c1", companyName: "Northwind", otherCompanies: false, account: "a1", name: "Dana Owner", email: "owner@vault.test", keys });
    assert.equal(unlocked()?.company, "c1");
    lock();
    lock();
    assert.equal(unlocked(), null);
    assert.equal(told, 1);
    stop();
  });

  it("locks fifteen minutes after unlocking when nothing is touched", () => {
    const clock = manualClock();
    const stop = lockWhenIdle(new EventTarget(), 15, clock);
    hold({ company: "c1", companyName: "Northwind", otherCompanies: false, account: "a1", name: "Dana Owner", email: "owner@vault.test", keys });
    clock.pass(15 * 60_000);
    assert.equal(unlocked(), null);
    stop();
  });

  it("locks after the idle stretch, counted from unlocking and restarted by input", () => {
    const clock = manualClock();
    const target = new EventTarget();
    const stop = lockWhenIdle(target, 15, clock);
    hold({ company: "c1", companyName: "Northwind", otherCompanies: false, account: "a1", name: "Dana Owner", email: "owner@vault.test", keys });
    clock.pass(14 * 60_000);
    target.dispatchEvent(new Event("keydown"));
    clock.pass(14 * 60_000);
    assert.equal(unlocked()?.company, "c1", "input restarted the count");
    clock.pass(60_000);
    assert.equal(unlocked(), null, "fifteen minutes with no input");
    stop();
  });
});

describe("saving a first key set", () => {
  it("is done once the keys are kept and the chain began", () => {
    assert.deepEqual(firstSave(null, true), { kind: "done" });
  });

  it("sends the person to unlock when the keys are kept but the chain did not begin, or were kept before", () => {
    const afterChain = firstSave(null, false);
    const existing = firstSave("keys_exist", true);
    assert.equal(afterChain.kind, "kept");
    assert.equal(existing.kind, "kept");
    assert.match(afterChain.kind === "kept" ? afterChain.words : "", /Unlock with the password you just set/);
    assert.match(existing.kind === "kept" ? existing.words : "", /already set an unlock password/);
  });

  it("is a plain failure for any other refusal", () => {
    assert.deepEqual(firstSave("unavailable", false), { kind: "failed" });
  });
});
