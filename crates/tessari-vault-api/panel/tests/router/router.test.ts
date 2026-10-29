// The panel's URL router: typed routes built from their patterns, any URL resolved (a deep link is a URL like any
// other), query state with declared types, guards run on every navigation including back and forward, and a redirect
// loop refused rather than followed for ever.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { integer, text } from "../../src/router/query.ts";
import { Router, defineRoutes, type Outcome } from "../../src/router/router.ts";
import { FakeHistory } from "./support.ts";

let signedIn = true;

const routes = defineRoutes({
  home: { path: "/" },
  signIn: { path: "/sign-in", query: { next: text } },
  records: {
    path: "/companies/:company/records",
    query: { cursor: text, limit: integer },
    guard: () => (signedIn ? { kind: "allow" } : { kind: "redirect", to: "#/sign-in" }),
  },
  record: { path: "/companies/:company/records/:record", query: { tab: text } },
  loopA: { path: "/a", guard: () => ({ kind: "redirect", to: "#/b" }) },
  loopB: { path: "/b", guard: () => ({ kind: "redirect", to: "#/a" }) },
});

function started(start: string): { history: FakeHistory; router: Router<typeof routes>; seen: Outcome<typeof routes>[] } {
  const history = new FakeHistory(start);
  const router = new Router(routes, history);
  const seen: Outcome<typeof routes>[] = [];
  router.start((outcome) => seen.push(outcome));
  return { history, router, seen };
}

describe("the panel router", () => {
  it("builds a route's link from its parameters and query, encoded, in a stable order", () => {
    const router = new Router(routes, new FakeHistory());
    assert.equal(router.href("record", { company: "c 1", record: "r/2" }, { tab: "files" }), "#/companies/c%201/records/r%2F2?tab=files");
    assert.equal(router.href("records", { company: "c1" }, { limit: 50, cursor: "x&y" }), "#/companies/c1/records?cursor=x%26y&limit=50");
    assert.equal(router.href("records", { company: "c1" }), "#/companies/c1/records");
  });

  it("resolves any URL to its route with decoded parameters and typed query, dropping keys it does not declare", () => {
    const router = new Router(routes, new FakeHistory());
    const match = router.resolve("#/companies/c%201/records/r%2F2?tab=files&spy=1");
    assert.deepEqual(match, { name: "record", params: { company: "c 1", record: "r/2" }, query: { tab: "files" } });
    assert.deepEqual(router.resolve("#/companies/c1/records?limit=abc"), {
      name: "records",
      params: { company: "c1" },
      query: { cursor: null, limit: null },
    });
    assert.equal(router.resolve("#/companies/c1/nowhere"), null);
    assert.equal(router.resolve("#/companies//records"), null);
  });

  it("answers a deep link when it starts, and each navigation after", () => {
    signedIn = true;
    const { router, seen } = started("#/companies/c1/records/r1");
    router.navigate("records", { company: "c1" }, { limit: 20 });
    assert.deepEqual(
      seen.map((outcome) => (outcome.kind === "matched" ? outcome.match.name : outcome.kind)),
      ["record", "records"],
    );
  });

  it("sends a guarded route elsewhere, on a link and on back and forward alike", () => {
    signedIn = true;
    const { history, seen } = started("#/companies/c1/records");
    const routerAt = (): string => history.current();
    signedIn = false;
    history.push("#/companies/c1/records/r1");
    history.back();
    assert.equal(routerAt(), "#/sign-in");
    const last = seen.at(-1);
    assert.equal(last?.kind === "matched" ? last.match.name : last?.kind, "signIn");
  });

  it("refuses a redirect loop instead of following it", () => {
    const { seen } = started("#/a");
    assert.equal(seen.at(-1)?.kind, "loop");
  });

  it("says a URL names no route", () => {
    const { seen } = started("#/nowhere");
    assert.deepEqual(seen.at(-1), { kind: "not-found", href: "#/nowhere" });
  });
});
