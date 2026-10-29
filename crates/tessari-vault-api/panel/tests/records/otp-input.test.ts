// What the edit form takes for a one-time password: the setup key an issuer shows (spaces and case ignored) or the
// otpauth:// link its QR code carries, read into the field's secret, algorithm, digits and period; anything else, a
// counter-based link included, is refused rather than guessed at.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { otpFromInput } from "../../src/records/otp-input.ts";

describe("a one-time password typed into the form", () => {
  it("takes a setup key, spaces and case ignored, with the usual 6 digits every 30 seconds", () => {
    assert.deepEqual(otpFromInput("jbsw y3dp ehpk 3pxp"), { secret: "JBSWY3DPEHPK3PXP", algorithm: "SHA1", digits: 6, period: 30 });
  });

  it("takes an otpauth:// link with its own algorithm, digits and period", () => {
    const link = "otpauth://totp/Northwind:dana%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=Northwind&algorithm=SHA256&digits=8&period=60";
    assert.deepEqual(otpFromInput(link), { secret: "JBSWY3DPEHPK3PXP", algorithm: "SHA256", digits: 8, period: 60 });
    assert.deepEqual(otpFromInput("otpauth://totp/x?secret=jbswy3dpehpk3pxp"), { secret: "JBSWY3DPEHPK3PXP", algorithm: "SHA1", digits: 6, period: 30 });
  });

  it("refuses a counter-based link, an algorithm it doesn't know, a link with no secret and a key that isn't base32", () => {
    assert.equal(otpFromInput("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=0"), null);
    assert.equal(otpFromInput("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=MD5"), null);
    assert.equal(otpFromInput("otpauth://totp/x?issuer=Northwind"), null);
    assert.equal(otpFromInput("not a key!"), null);
    assert.equal(otpFromInput("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=7"), null);
  });
});
