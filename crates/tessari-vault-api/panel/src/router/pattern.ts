// A route's path pattern, `/companies/:company/records/:record`: the parameter names are read from the pattern by the
// type system, so a link cannot leave one out, and every value is percent-encoded on the way out and decoded on the
// way in, so a value holding `/` or `?` stays one segment.

/** The names of the `:name` segments of `P`. */
export type ParamNames<P extends string> = P extends `${string}:${infer Name}/${infer Rest}`
  ? Name | ParamNames<`/${Rest}`>
  : P extends `${string}:${infer Name}`
    ? Name
    : never;

/** The parameters a link to `P` needs. */
export type Params<P extends string> = { readonly [Name in ParamNames<P>]: string };

function segments(path: string): string[] {
  return path.split("/").filter((segment, index) => index > 0 || segment !== "");
}

/** The path `pattern` names with `params` filled in. */
export function build(pattern: string, params: Readonly<Record<string, string>>): string {
  const filled = segments(pattern).map((segment) =>
    segment.startsWith(":") ? encodeURIComponent(params[segment.slice(1)] ?? "") : segment,
  );
  return `/${filled.join("/")}`;
}

/** The parameters `path` holds for `pattern`, or `null` when it is not that pattern or a parameter is empty. */
export function match(pattern: string, path: string): Record<string, string> | null {
  const wanted = segments(pattern);
  const given = segments(path);
  if (wanted.length !== given.length) {
    return null;
  }
  const params: Record<string, string> = {};
  for (const [index, segment] of wanted.entries()) {
    const value = given[index] ?? "";
    if (segment.startsWith(":")) {
      if (value === "") {
        return null;
      }
      try {
        params[segment.slice(1)] = decodeURIComponent(value);
      } catch {
        return null;
      }
    } else if (segment !== value) {
      return null;
    }
  }
  return params;
}
