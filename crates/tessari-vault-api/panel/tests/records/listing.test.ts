// The records list without a DOM: which rows a filter keeps, the words for a record that would not open, the type names,
// and the clipboard that clears itself after a copy.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { COPY_CLEARS_AFTER_MS, copier, matches, typeName, unreadableWords, type Listed } from "../../src/records/listing.ts";
import type { RecordRow } from "../../src/records-model.ts";

const row = (recordType: string): RecordRow => ({
  id: "1".repeat(32),
  recordType,
  revision: 1,
  keyId: "2".repeat(32),
  author: "3".repeat(32),
  created: 1,
  updated: 2,
  rotationDue: false,
});
const login: Listed = { row: row("login"), document: { v: 1, type: "login", title: "Mail Admin", sections: [] } };
const note: Listed = { row: row("secure-note"), document: { v: 1, type: "secure-note", title: "Door codes", sections: [] } };
const broken: Listed = { row: row("login"), problem: "bad-signature" };

describe("the records filter", () => {
  it("keeps every record for an empty query and no type", () => {
    assert.deepEqual([login, note, broken].filter((item) => matches(item, "  ", null)), [login, note, broken]);
  });

  it("matches the title case-insensitively and the type exactly", () => {
    assert.deepEqual([login, note, broken].filter((item) => matches(item, "mail", null)), [login]);
    assert.deepEqual([login, note, broken].filter((item) => matches(item, "", "secure-note")), [note]);
    assert.deepEqual([login, note, broken].filter((item) => matches(item, "", "login")), [login, broken]);
    assert.deepEqual([login, note, broken].filter((item) => matches(item, "door", "login")), []);
  });
});

describe("the words for a record that would not open", () => {
  it("says why for each kind of refusal, and never shows the raw reason", () => {
    assert.match(unreadableWords("unvouched-signer"), /owner hasn't vouched for/);
    assert.match(unreadableWords("bad-signature"), /signature doesn't match/);
    assert.match(unreadableWords("unauthentic"), /couldn't open it with your keys/);
    assert.match(unreadableWords("key-mismatch"), /couldn't open it with your keys/);
    assert.match(unreadableWords("unvouched-recipient"), /group key/);
    assert.match(unreadableWords("malformed"), /can't read/);
    for (const reason of ["unvouched-signer", "bad-signature", "unauthentic", "malformed"] as const) {
      assert.doesNotMatch(unreadableWords(reason), new RegExp(reason));
    }
  });
});

describe("type names", () => {
  it("names a built-in type and shows a company type by its id", () => {
    assert.equal(typeName("login"), "Login");
    assert.equal(typeName("secure-note"), "Secure note");
    assert.equal(typeName("acme:badge"), "acme:badge");
  });
});

describe("copying", () => {
  /** A clock the test advances by hand, and a clipboard it can read. */
  function harness(fails = false) {
    let now = 0;
    const timers = new Map<number, { at: number; run: () => void }>();
    let next = 1;
    const clock = {
      set: (run: () => void, ms: number): unknown => {
        timers.set(next, { at: now + ms, run });
        return next++;
      },
      clear: (handle: unknown): void => void timers.delete(Number(handle)),
    };
    const advance = (ms: number): void => {
      now += ms;
      for (const [handle, timer] of [...timers]) {
        if (timer.at <= now) {
          timers.delete(handle);
          timer.run();
        }
      }
    };
    const board = { text: "something else", writes: 0 };
    const clipboard = {
      write: async (text: string): Promise<void> => {
        if (fails) {
          throw new Error("denied");
        }
        board.text = text;
        board.writes += 1;
      },
    };
    const said: string[] = [];
    return { copy: copier(clipboard, clock, (words) => said.push(words)), advance, board, said, timers };
  }

  it("copies, says when the clipboard will clear, and clears it after 30 seconds", async () => {
    const { copy, advance, board, said } = harness();
    await copy.copy("hunter2", "Password");
    assert.equal(board.text, "hunter2");
    assert.equal(said.at(-1), "Password copied. The clipboard clears in 30 seconds.");
    advance(COPY_CLEARS_AFTER_MS - 1);
    assert.equal(board.text, "hunter2");
    advance(1);
    await new Promise((settled) => setImmediate(settled));
    assert.equal(board.text, "");
    assert.equal(said.at(-1), "Clipboard cleared.");
    assert.equal(COPY_CLEARS_AFTER_MS, 30_000);
  });

  it("restarts the count on a newer copy, and clears at once when told to", async () => {
    const { copy, advance, board, timers } = harness();
    await copy.copy("first", "Password");
    advance(20_000);
    await copy.copy("second", "Username");
    advance(20_000);
    assert.equal(board.text, "second");
    assert.equal(timers.size, 1);
    await copy.clearNow();
    assert.equal(board.text, "");
    assert.equal(timers.size, 0);
  });

  it("does not touch the clipboard when nothing was copied", async () => {
    const { copy, board } = harness();
    await copy.clearNow();
    assert.equal(board.text, "something else");
    assert.equal(board.writes, 0);
  });

  it("says so when the browser refuses the copy, and sets no timer", async () => {
    const { copy, said, timers } = harness(true);
    await copy.copy("hunter2", "Password");
    assert.equal(said.at(-1), "This browser didn't allow copying. Show the value and copy it yourself.");
    assert.equal(timers.size, 0);
  });
});
