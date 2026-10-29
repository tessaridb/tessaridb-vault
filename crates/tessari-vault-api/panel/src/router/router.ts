// The panel's URL router, a module of its own so any page of the family can use it: named routes declared once with
// their path pattern, query state and guard; links built from a route's name, so a parameter cannot be left out; any
// URL resolved to its route, which is what makes a deep link work; and every navigation (a link, back, forward, a
// typed URL) passed through the target's guard, which may send it elsewhere. A chain of redirects is cut at a ceiling
// and reported, never followed for ever.

import type { HistoryPort } from "./history.ts";
import { build, match, type Params } from "./pattern.ts";
import { readQuery, writeQuery, type QueryInput, type QuerySpec, type QueryValues } from "./query.ts";

/** What a guard decides about the navigation it is asked about. */
export type Verdict = { readonly kind: "allow" } | { readonly kind: "redirect"; readonly to: string };

/** One route: its path pattern, the query keys it reads, and who may reach it. */
export interface RouteSpec {
  readonly path: string;
  readonly query?: QuerySpec;
  readonly guard?: () => Verdict;
}

export type RouteTable = Readonly<Record<string, RouteSpec>>;

/** The routes as declared, with their patterns kept as literal types. */
export function defineRoutes<const R extends RouteTable>(routes: R): R {
  return routes;
}

/** A URL resolved: the route's name, its decoded parameters and its declared query values. */
export type Match<R extends RouteTable> = {
  [N in keyof R & string]: {
    readonly name: N;
    readonly params: Params<R[N]["path"]>;
    readonly query: QueryValues<R[N]["query"]>;
  };
}[keyof R & string];

/** What a navigation came to. */
export type Outcome<R extends RouteTable> =
  | { readonly kind: "matched"; readonly match: Match<R> }
  | { readonly kind: "not-found"; readonly href: string }
  | { readonly kind: "loop"; readonly href: string };

/** The most redirects one navigation follows. */
const REDIRECT_CEILING = 8;

function split(href: string): { path: string; query: string } {
  const bare = href.startsWith("#") ? href.slice(1) : href;
  const at = bare.indexOf("?");
  const path = at === -1 ? bare : bare.slice(0, at);
  return { path: path === "" ? "/" : path, query: at === -1 ? "" : bare.slice(at + 1) };
}

export class Router<R extends RouteTable> {
  readonly #routes: R;
  readonly #history: HistoryPort;
  #listener: ((outcome: Outcome<R>) => void) | undefined;

  constructor(routes: R, history: HistoryPort) {
    this.#routes = routes;
    this.#history = history;
  }

  /** The link to route `name`. */
  href<N extends keyof R & string>(name: N, params: Params<R[N]["path"]>, query?: QueryInput<R[N]["query"]>): string {
    const spec = this.#routes[name];
    if (spec === undefined) {
      throw new RangeError(`no route named ${name}`);
    }
    return `#${build(spec.path, params)}${writeQuery(spec.query, query ?? {})}`;
  }

  /** The route `href` names, or `null` when it names none. */
  resolve(href: string): Match<R> | null {
    const { path, query } = split(href);
    for (const [name, spec] of Object.entries(this.#routes)) {
      const params = match(spec.path, path);
      if (params !== null) {
        const resolved = { name, params, query: readQuery(spec.query, query) };
        if (this.#isMatch(resolved)) {
          return resolved;
        }
      }
    }
    return null;
  }

  /** Answer the current URL now and every later one, through `listener`; the returned call stops listening. */
  start(listener: (outcome: Outcome<R>) => void): () => void {
    this.#listener = listener;
    const stop = this.#history.subscribe(() => this.#settle());
    this.#settle();
    return () => {
      stop();
      this.#listener = undefined;
    };
  }

  /** Go to route `name`, as a link does. */
  navigate<N extends keyof R & string>(name: N, params: Params<R[N]["path"]>, query?: QueryInput<R[N]["query"]>): void {
    this.#history.push(this.href(name, params, query));
    this.#settle();
  }

  /** Resolve the current URL, following its guards' redirects up to the ceiling, and report what it came to. */
  #settle(): void {
    let href = this.#history.current();
    for (let followed = 0; followed <= REDIRECT_CEILING; followed += 1) {
      const resolved = this.resolve(href);
      if (resolved === null) {
        this.#listener?.({ kind: "not-found", href });
        return;
      }
      const verdict = this.#routes[resolved.name]?.guard?.() ?? { kind: "allow" };
      if (verdict.kind === "allow") {
        this.#listener?.({ kind: "matched", match: resolved });
        return;
      }
      this.#history.replace(verdict.to);
      href = verdict.to;
    }
    this.#listener?.({ kind: "loop", href });
  }

  /** Whether `value` is a match of a declared route: its parameters are exactly the ones its pattern names. */
  #isMatch(value: { name: string; params: Record<string, string>; query: Record<string, unknown> }): value is Match<R> {
    const spec = this.#routes[value.name];
    if (spec === undefined) {
      return false;
    }
    const names = spec.path
      .split("/")
      .filter((segment) => segment.startsWith(":"))
      .map((segment) => segment.slice(1));
    const given = Object.keys(value.params);
    return names.length === given.length && names.every((name) => given.includes(name));
  }
}
