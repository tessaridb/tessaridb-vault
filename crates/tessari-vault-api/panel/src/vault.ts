// The keys this page holds open: one company's key set, from unlocking until it locks. It locks when the person presses
// Lock, after a stretch with no input, when they sign out, and when the tab closes (the keys live only in this page's
// memory). Locking drops the page's only references to the keys; they are non-extractable, so their bytes were never
// within the page's reach.

import type { OpenKeySet } from "./crypto/key-set.ts";

/** An unlocked company: whose keys, in which company. */
export interface Unlocked {
  readonly company: string;
  readonly account: string;
  readonly keys: OpenKeySet;
}

/** How long the vault stays unlocked with no input. */
export const IDLE_MINUTES = 15;

let held: Unlocked | null = null;
const listeners = new Set<() => void>();
/** Starts the idle count again; set by `lockWhenIdle`, called on unlocking so the count runs before any input. */
let arm: (() => void) | null = null;

/** The unlocked company, or null when the vault is locked. */
export const unlocked = (): Unlocked | null => held;

/** Holds `opened` until the vault locks. */
export function hold(opened: Unlocked): void {
  held = opened;
  arm?.();
}

/** Locks the vault and tells whoever is listening, so a view showing records can clear itself. */
export function lock(): void {
  if (held === null) {
    return;
  }
  held = null;
  for (const listener of listeners) {
    listener();
  }
}

/** Calls `listener` whenever the vault locks; the returned call stops. */
export function onLock(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * Locks after `minutes` with none of `events` on `target`: every input restarts the count. Timers come from `clock`
 * so the tests drive them.
 */
export function lockWhenIdle(
  target: EventTarget,
  minutes: number,
  clock: { readonly set: (run: () => void, ms: number) => unknown; readonly clear: (handle: unknown) => void },
  events: readonly string[] = ["pointerdown", "keydown", "wheel", "touchstart"],
): () => void {
  let timer: unknown = null;
  const restart = (): void => {
    if (timer !== null) {
      clock.clear(timer);
    }
    timer = held === null ? null : clock.set(lock, minutes * 60_000);
  };
  for (const name of events) {
    target.addEventListener(name, restart, { passive: true });
  }
  arm = restart;
  restart();
  return () => {
    arm = null;
    if (timer !== null) {
      clock.clear(timer);
    }
    for (const name of events) {
      target.removeEventListener(name, restart);
    }
  };
}
