// The phone menu: the button opens and closes it, and going somewhere, Escape or a tap outside closes it.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { nextMenu } from "../../src/menu.ts";

describe("the phone menu", () => {
  it("the button opens it and closes it again", () => {
    assert.equal(nextMenu(false, "toggle"), true);
    assert.equal(nextMenu(true, "toggle"), false);
  });

  it("closes on going somewhere, on Escape and on a tap outside, and those never open it", () => {
    for (const event of ["navigate", "escape", "outside"] as const) {
      assert.equal(nextMenu(true, event), false, event);
      assert.equal(nextMenu(false, event), false, event);
    }
  });
});
