// One-time password fields: RFC 6238 Appendix B (all 18 rows, 8 digits) and RFC 4648 §10 base32. The SHA-256 and
// SHA-512 rows use the 32- and 64-byte seeds of the RFC's Appendix A reference code (errata 2866), not the 20-byte
// string its prose names. Values copied from the RFC texts retrieved from rfc-editor.org on 2026-10-28.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { utf8 } from "../../src/crypto/bytes.ts";
import { base32Decode, base32Encode, totpCode, type OtpAlgorithm } from "../../src/records/totp.ts";
import { refusedWith } from "../crypto/support.ts";

const SEEDS: Record<OtpAlgorithm, string> = {
  SHA1: "12345678901234567890",
  SHA256: "12345678901234567890123456789012",
  SHA512: "1234567890123456789012345678901234567890123456789012345678901234",
};

const ROWS: [number, string, string, string][] = [
  [59, "94287082", "46119246", "90693936"],
  [1111111109, "07081804", "68084774", "25091201"],
  [1111111111, "14050471", "67062674", "99943326"],
  [1234567890, "89005924", "91819424", "93441116"],
  [2000000000, "69279037", "90698825", "38618901"],
  [20000000000, "65353130", "77737706", "47863826"],
];

describe("TOTP (RFC 6238 Appendix B)", () => {
  for (const [time, sha1, sha256, sha512] of ROWS) {
    it(`matches every mode at T = ${time}`, async () => {
      for (const [algorithm, expected] of [
        ["SHA1", sha1],
        ["SHA256", sha256],
        ["SHA512", sha512],
      ] as const) {
        const secret = utf8(SEEDS[algorithm]);
        assert.equal(await totpCode({ secret, algorithm, digits: 8, period: 30 }, time), expected, algorithm);
      }
    });
  }

  it("answers the last six digits for a six-digit field", async () => {
    const code = await totpCode({ secret: utf8(SEEDS.SHA1), algorithm: "SHA1", digits: 6, period: 30 }, 59);
    assert.equal(code, "287082");
  });

  it("refuses a period that is not a positive whole number of seconds", async () => {
    await refusedWith(totpCode({ secret: utf8(SEEDS.SHA1), algorithm: "SHA1", digits: 6, period: 0 }, 59), "malformed");
  });
});

const BASE32: [string, string][] = [
  ["", ""],
  ["f", "MY======"],
  ["fo", "MZXQ===="],
  ["foo", "MZXW6==="],
  ["foob", "MZXW6YQ="],
  ["fooba", "MZXW6YTB"],
  ["foobar", "MZXW6YTBOI======"],
];

describe("base32 (RFC 4648 §10)", () => {
  for (const [text, encoded] of BASE32) {
    it(`encodes and decodes "${text}"`, () => {
      assert.equal(base32Encode(utf8(text)), encoded);
      assert.equal(new TextDecoder().decode(base32Decode(encoded)), text);
    });
  }

  it("reads a seed as people paste it: lower case, spaces, no padding", () => {
    assert.equal(new TextDecoder().decode(base32Decode("mzxw 6ytb oi")), "foobar");
  });

  it("refuses a character outside the alphabet and bits left over", () => {
    assert.throws(() => base32Decode("MZXW1"), /malformed/);
    assert.throws(() => base32Decode("MZXW6YTBOJ"), /malformed/);
  });
});
