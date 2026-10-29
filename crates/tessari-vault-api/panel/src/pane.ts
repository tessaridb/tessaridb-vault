// A view is drawn after its data arrives, and by then the route may have moved on. Each route is drawn into a pane of
// its own that replaces the previous one, so a draw that finishes late fills a pane no longer on the page.

/** The element a view is drawn into: it holds one pane at a time. */
interface Holder<Pane> {
  replaceChildren(...nodes: Pane[]): void;
}

/** A new pane from `make`, put in `view` in place of whatever it held. */
export function freshPane<Pane>(view: Holder<NoInfer<Pane>>, make: () => Pane): Pane {
  const pane = make();
  view.replaceChildren(pane);
  return pane;
}
