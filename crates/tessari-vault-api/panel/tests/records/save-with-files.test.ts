// Saving with files chosen in the form, without a DOM: the files are sent before the revision, and a revision that is
// not kept deletes every file it sent and puts the form back as it was.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Control, Settled } from "../../src/records/custom-fields-view.ts";
import { saveWithFiles } from "../../src/records/save-with-files.ts";
import { BUILT_IN_TYPES, recordFromTemplate } from "../../src/records/types.ts";

const login = BUILT_IN_TYPES[0];
if (login === undefined) {
  throw new Error("no built-in type");
}
const DOCUMENT = recordFromTemplate(login, "t");

function fileControl(log: string[], answer: Settled): Pick<Control, "settle" | "undo"> {
  return {
    settle: () => {
      log.push("send");
      return Promise.resolve(answer);
    },
    undo: () => log.push("undo"),
  };
}

const attach = (log: string[]) => ({
  send: () => Promise.reject(new Error("not used")),
  discard: (id: string) => {
    log.push(`discard ${id}`);
    return Promise.resolve();
  },
});

describe("saving a record with its chosen files", () => {
  it("sends the files, then saves, and deletes nothing when the save is kept", async () => {
    const log: string[] = [];
    const controls = [fileControl(log, { ok: true, sent: "f1" }), fileControl(log, { ok: true, sent: null })];
    const refused = await saveWithFiles(controls, attach(log), () => ({ ok: true, document: DOCUMENT }), () => {
      log.push("save");
      return Promise.resolve(null);
    });
    assert.equal(refused, null);
    assert.deepEqual(log, ["send", "send", "save"]);
  });

  it("deletes the files it sent and puts the form back when the save is refused", async () => {
    const log: string[] = [];
    const controls = [fileControl(log, { ok: true, sent: "f1" }), fileControl(log, { ok: true, sent: "f2" })];
    const refused = await saveWithFiles(controls, attach(log), () => ({ ok: true, document: DOCUMENT }), () => Promise.resolve("refused words"));
    assert.equal(refused, "refused words");
    assert.deepEqual(log, ["send", "send", "discard f1", "discard f2", "undo", "undo"]);
  });

  it("stops at a file that could not be sent, deleting the ones sent before it, and saves nothing", async () => {
    const log: string[] = [];
    const controls = [fileControl(log, { ok: true, sent: "f1" }), fileControl(log, { ok: false, words: "too big" })];
    const refused = await saveWithFiles(controls, attach(log), () => ({ ok: true, document: DOCUMENT }), () => {
      log.push("save");
      return Promise.resolve(null);
    });
    assert.equal(refused, "too big");
    assert.deepEqual(log, ["send", "send", "discard f1", "undo", "undo"]);
  });
});
