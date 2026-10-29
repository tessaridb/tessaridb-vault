// Where the router reads and writes the current URL. The browser's is the location hash, so every view is a link that
// reloads to itself and needs no route on the server; tests use a history they drive by hand.

export interface HistoryPort {
  /** The current hash, `#/path?query`. */
  current(): string;
  /** A new entry, as a link does. */
  push(href: string): void;
  /** The current entry rewritten, as a redirect does. */
  replace(href: string): void;
  /** Called when the entry changes from outside: back, forward, a typed URL. */
  subscribe(listener: () => void): () => void;
}

/** The browser window's history, by its location hash. */
export function browserHistory(window: Window): HistoryPort {
  return {
    current: () => window.location.hash || "#/",
    push: (href) => window.history.pushState(null, "", href),
    replace: (href) => window.history.replaceState(null, "", href),
    subscribe: (listener) => {
      window.addEventListener("popstate", listener);
      return () => window.removeEventListener("popstate", listener);
    },
  };
}
