// Query state a route declares: each key has a codec, a value that does not parse reads as absent, and a key the route
// does not declare is dropped, so a link cannot smuggle state into a view that never asked for it.

/** How one query value is read, and written when it is of the codec's type. */
export interface Codec<T> {
  parse(raw: string): T | null;
  write(value: unknown): string | null;
}

/** Any text. */
export const text: Codec<string> = {
  parse: (raw) => raw,
  write: (value) => (typeof value === "string" ? value : null),
};

/** A whole number, written in decimal. */
export const integer: Codec<number> = {
  parse: (raw) => (/^-?\d{1,15}$/.test(raw) ? Number(raw) : null),
  write: (value) => (typeof value === "number" && Number.isSafeInteger(value) ? String(value) : null),
};

/** A route's declared query keys. */
export type QuerySpec = Readonly<Record<string, Codec<string> | Codec<number>>>;

/** The values a query spec reads as: every declared key, `null` where absent. */
export type QueryValues<Q extends QuerySpec | undefined> = Q extends QuerySpec
  ? { readonly [Key in keyof Q]: Q[Key] extends Codec<infer T> ? T | null : never }
  : Record<string, never>;

/** The values a link may set: any declared key, omitted or `null` meaning absent. */
export type QueryInput<Q extends QuerySpec | undefined> = Q extends QuerySpec
  ? { readonly [Key in keyof Q]?: Q[Key] extends Codec<infer T> ? T | null : never }
  : Record<string, never>;

/** The declared values of the query string `raw`. */
export function readQuery(spec: QuerySpec | undefined, raw: string): Record<string, unknown> {
  const given = new URLSearchParams(raw);
  const values: Record<string, unknown> = {};
  for (const [key, codec] of Object.entries(spec ?? {})) {
    const value = given.get(key);
    values[key] = value === null ? null : codec.parse(value);
  }
  return values;
}

/** The query string for `values`, declared keys only, in the spec's order, empty when nothing is set. */
export function writeQuery(spec: QuerySpec | undefined, values: Readonly<Record<string, unknown>>): string {
  const pairs: string[] = [];
  for (const [key, codec] of Object.entries(spec ?? {})) {
    const written = codec.write(values[key]);
    if (written !== null) {
      pairs.push(`${key}=${encodeURIComponent(written)}`);
    }
  }
  return pairs.length === 0 ? "" : `?${pairs.join("&")}`;
}
