import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { sshDownloadName } from "../../src/records/ssh-download.ts";

describe("SSH key downloads", () => {
  it("names the public and private files from the field label", () => {
    assert.equal(sshDownloadName("Deploy key", "public"), "deploy-key.pub");
    assert.equal(sshDownloadName("Deploy key", "private"), "deploy-key");
    assert.equal(sshDownloadName("  ", "private"), "id-ed25519");
  });
});
