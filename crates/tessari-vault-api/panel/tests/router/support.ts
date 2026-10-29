// A history the tests drive by hand: entries, a position, and the listeners a browser would call on back and forward.

import type { HistoryPort } from "../../src/router/history.ts";

export class FakeHistory implements HistoryPort {
  readonly #entries: string[];
  #at = 0;
  readonly #listeners = new Set<() => void>();

  constructor(start = "#/") {
    this.#entries = [start];
  }

  current(): string {
    return this.#entries[this.#at] ?? "#/";
  }

  push(href: string): void {
    this.#entries.splice(this.#at + 1, this.#entries.length, href);
    this.#at += 1;
  }

  replace(href: string): void {
    this.#entries[this.#at] = href;
  }

  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** The browser's back button: the position moves and the listeners hear of it. */
  back(): void {
    this.#at = Math.max(0, this.#at - 1);
    this.#listeners.forEach((listener) => listener());
  }

  forward(): void {
    this.#at = Math.min(this.#entries.length - 1, this.#at + 1);
    this.#listeners.forEach((listener) => listener());
  }

  get length(): number {
    return this.#entries.length;
  }
}
