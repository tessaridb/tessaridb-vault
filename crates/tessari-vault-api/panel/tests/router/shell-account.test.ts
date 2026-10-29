// The shell says who is signed in: a block at the top of the sidebar's foot, which the phone menu also shows, hidden
// until a company is unlocked and then filled with the person's name and email.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const markup = readFileSync(new URL("../../assets/index.html", import.meta.url), "utf8");

describe("who is signed in", () => {
  it("has a place at the top of the sidebar's foot, hidden until unlocked", () => {
    const foot = markup.slice(markup.indexOf('<div class="side-foot">'));
    assert.match(foot, /^<div class="side-foot">\s*<div id="account" class="account" hidden>/);
    assert.match(foot, /<p class="caption">Signed in as<\/p>\s*<p id="account-name" class="account-name"><\/p>\s*<p id="account-email" class="account-email"><\/p>/);
  });
});
