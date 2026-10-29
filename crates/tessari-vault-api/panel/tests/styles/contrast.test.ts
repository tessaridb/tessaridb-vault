// The colour tokens as the page paints them: every text pair the stylesheets use reaches WCAG AA (4.5:1) in both
// themes, a translucent wash is measured over each surface it sits on, and the two copies of the dark theme agree.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const TOKENS = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");

/** The `--name: value` declarations of the first block whose opening text matches `opening`. */
function block(opening: RegExp): Map<string, string> {
  const start = TOKENS.search(opening);
  assert.ok(start >= 0, `no block ${opening}`);
  const body = TOKENS.slice(TOKENS.indexOf("{", start) + 1, TOKENS.indexOf("}", start));
  return new Map([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((match) => [match[1] ?? "", (match[2] ?? "").trim()]));
}

const PRIMITIVE = block(/:root \{\s*--violet/);
const LIGHT = block(/:root \{\s*color-scheme: light/);
const DARK = block(/:root\[data-theme="dark"\] \{/);
const DARK_BY_PREFERENCE = block(/:root:not\(\[data-theme="light"\]\) \{/);

type Rgba = readonly [number, number, number, number];

/** An `oklch(L C H [/ A])` value as gamma-encoded sRGB in 0..1, through OKLab. */
function rgba(value: string): Rgba {
  const match = /^oklch\(([\d.]+) ([\d.]+) ([\d.]+)(?: \/ ([\d.]+))?\)$/.exec(value);
  assert.ok(match, `not an oklch colour: ${value}`);
  const [lightness, chroma, hue, alpha] = [Number(match[1]), Number(match[2]), Number(match[3]), Number(match[4] ?? 1)];
  const a = chroma * Math.cos((hue * Math.PI) / 180);
  const b = chroma * Math.sin((hue * Math.PI) / 180);
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const linear = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  const encode = (c: number): number => {
    const clamped = Math.min(1, Math.max(0, c));
    return clamped <= 0.0031308 ? 12.92 * clamped : 1.055 * clamped ** (1 / 2.4) - 0.055;
  };
  return [encode(linear[0] ?? 0), encode(linear[1] ?? 0), encode(linear[2] ?? 0), alpha];
}

/** Semantic token `name` in `theme`, followed through `var()` to its primitive. */
function token(theme: ReadonlyMap<string, string>, name: string): string {
  const value = theme.get(name) ?? PRIMITIVE.get(name);
  assert.ok(value !== undefined, `no token ${name}`);
  const reference = /^var\((--[\w-]+)\)$/.exec(value);
  return reference ? token(theme, reference[1] ?? "") : value;
}

/** `layers` painted bottom to top, as the browser blends them in sRGB. */
function painted(theme: ReadonlyMap<string, string>, ...layers: readonly string[]): Rgba {
  return layers.map((name) => rgba(token(theme, name))).reduce((under, over) => {
    const alpha = over[3];
    return [0, 1, 2].map((i) => (over[i] ?? 0) * alpha + (under[i] ?? 0) * (1 - alpha)).concat(1) as unknown as Rgba;
  });
}

function ratio(first: Rgba, second: Rgba): number {
  const luminance = (c: Rgba): number => {
    const linear = c.slice(0, 3).map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * (linear[0] ?? 0) + 0.7152 * (linear[1] ?? 0) + 0.0722 * (linear[2] ?? 0);
  };
  const [lighter, darker] = [luminance(first), luminance(second)].sort((x, y) => y - x);
  return ((lighter ?? 0) + 0.05) / ((darker ?? 0) + 0.05);
}

const SURFACES = ["--surface", "--surface-raised", "--surface-sunken", "--side"];

/** Every text colour over every background it is painted on: [text, ...background layers bottom to top]. */
const PAIRS: readonly (readonly string[])[] = [
  ...["--text", "--text-muted", "--accent-text", "--bad"].flatMap((text) => SURFACES.map((surface) => [text, surface])),
  ...SURFACES.map((surface) => ["--accent-text", surface, "--accent-wash"]),
  ["--on-accent", "--accent"],
  ["--on-accent", "--accent-hover"],
  ["--on-bad", "--bad"],
];

describe("colour tokens", () => {
  for (const [label, theme] of [["light", LIGHT], ["dark", DARK]] as const) {
    it(`reach 4.5:1 for every text pair in the ${label} theme`, () => {
      const short = PAIRS.map(([text, ...under]) => ({ pair: `${text} on ${under.join(" + ")}`, ratio: ratio(painted(theme, text ?? ""), painted(theme, ...under)) }))
        .filter((each) => each.ratio < 4.5)
        .map((each) => `${each.pair} = ${each.ratio.toFixed(2)}`);
      assert.deepEqual(short, []);
    });
  }

  it("measure the stylesheet's own colours, so a failing pair is visible", () => {
    assert.ok(ratio(painted(LIGHT, "--text-muted"), painted(LIGHT, "--text-muted")) < 1.01);
    assert.ok(ratio(painted(LIGHT, "--text"), painted(LIGHT, "--surface")) > 15);
  });

  it("write the dark theme the same way twice", () => {
    assert.deepEqual([...DARK_BY_PREFERENCE].filter(([name]) => name !== "color-scheme"), [...DARK]);
  });
});
