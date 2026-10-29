// A view finishes drawing after its data arrives, and the route may have moved on by then. Each route is drawn into
// a pane of its own, so a draw that finishes late writes into a pane no longer on the page.

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { freshPane } from "../../src/pane.ts";

class Node {
  readonly children: Node[] = [];
  readonly name: string;
  constructor(name: string) {
    this.name = name;
  }
  replaceChildren(...nodes: Node[]): void {
    this.children.splice(0, this.children.length, ...nodes);
  }
}

describe("a pane per route", () => {
  it("a draw that finishes after the next route began does not reach the page", () => {
    const view = new Node("main");
    let made = 0;
    const make = (): Node => new Node(`pane ${(made += 1)}`);

    const list = freshPane(view, make);
    const record = freshPane(view, make);
    record.replaceChildren(new Node("the record"));
    // The list's data arrives last.
    list.replaceChildren(new Node("the list"));

    assert.deepEqual(
      view.children.map((pane) => pane.name),
      ["pane 2"],
    );
    assert.deepEqual(
      view.children.flatMap((pane) => pane.children.map((child) => child.name)),
      ["the record"],
    );
  });
});
