// The records list without a DOM: an opened record or the reason it would not open, the filter, the words a refusal
// is shown in, type names, and the clipboard that clears itself after a copy. All of it is tested.

import type { Refusal } from "../crypto/errors.ts";
import type { RecordRow } from "../records-model.ts";
import type { RecordDocument } from "./document.ts";
import { BUILT_IN_TYPES } from "./types.ts";

/** A record of the list: opened, or refused with the reason it would not open. */
export type Listed = { readonly row: RecordRow; readonly document: RecordDocument } | { readonly row: RecordRow; readonly problem: Refusal };

/** Whether `item` is kept by a title query and a type; a record that would not open is kept only with no query. */
export function matches(item: Listed, query: string, type: string | null): boolean {
  if (type !== null && item.row.recordType !== type) {
    return false;
  }
  const wanted = query.trim().toLocaleLowerCase();
  if (wanted === "") {
    return true;
  }
  return "document" in item && item.document.title.toLocaleLowerCase().includes(wanted);
}

/** Why a record is not shown, in words: what happened and what to do. */
export function unreadableWords(problem: Refusal): string {
  switch (problem) {
    case "unvouched-signer":
      return "It was written by someone your company's owner hasn't vouched for, so it isn't shown.";
    case "bad-signature":
      return "Its author's signature doesn't match, so it isn't shown. Ask its author to save it again.";
    case "unauthentic":
    case "key-mismatch":
    case "unknown-key":
      return "This page couldn't open it with your keys. Ask whoever shared it with you to share it again.";
    case "unvouched-recipient":
      return "It's shared through a group key your company's owner hasn't vouched for, so it isn't opened.";
    default:
      return "It's in a form this page can't read. Reload the page, and tell your admin if it stays.";
  }
}

/** A type's name: a built-in type's, or a company type's id. */
export const typeName = (id: string): string => BUILT_IN_TYPES.find((template) => template.id === id)?.name ?? id;

/** How long a copied value stays on the clipboard. */
export const COPY_CLEARS_AFTER_MS = 30_000;

interface Clock {
  readonly set: (run: () => void, ms: number) => unknown;
  readonly clear: (handle: unknown) => void;
}

/**
 * Copies values and clears the clipboard 30 seconds after the latest copy, or at once when the vault locks. A newer
 * copy restarts the count, so the clipboard is never cleared under a value copied a moment ago.
 */
export function copier(clipboard: { readonly write: (text: string) => Promise<void> }, clock: Clock, say: (words: string) => void) {
  let pending: unknown = null;
  const clear = async (): Promise<void> => {
    pending = null;
    await clipboard.write("").catch(() => undefined);
    say("Clipboard cleared.");
  };
  return {
    async copy(value: string, label: string): Promise<void> {
      try {
        await clipboard.write(value);
      } catch {
        say("This browser didn't allow copying. Show the value and copy it yourself.");
        return;
      }
      if (pending !== null) {
        clock.clear(pending);
      }
      pending = clock.set(() => void clear(), COPY_CLEARS_AFTER_MS);
      say(`${label} copied. The clipboard clears in 30 seconds.`);
    },
    async clearNow(): Promise<void> {
      if (pending === null) {
        return;
      }
      clock.clear(pending);
      await clear();
    },
  };
}
