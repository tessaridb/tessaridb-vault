// The shell's markup names each sidebar control's icon in `data-icon`; every name it uses is one the panel draws, so no
// link or button is left without its icon.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

import { isIconName } from "../../src/icons.ts";

const markup = readFileSync(new URL("../../assets/index.html", import.meta.url), "utf8");
const named = [...markup.matchAll(/data-icon="([^"]+)"/g)].map((match) => match[1]);

describe("the shell's icons", () => {
  it("draws every icon the markup names", () => {
    assert.ok(named.length >= 8, `found ${named.length} data-icon names`);
    for (const name of named) {
      assert.ok(isIconName(name), `"${name}" is named in the markup but not drawn`);
    }
  });

  it("refuses a name it has no icon for", () => {
    assert.equal(isIconName("nope"), false);
    assert.equal(isIconName(undefined), false);
  });
});
