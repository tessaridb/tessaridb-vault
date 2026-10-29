// Saving a record with the files chosen in its form: each chosen file is sealed and sent first, then the revision that
// names them; a revision that is not kept deletes the files it sent, so the vault keeps no file the record does not name.

import type { Control } from "./custom-fields-view.ts";
import type { RecordDocument } from "./document.ts";
import type { edited } from "./editing.ts";
import type { Attach } from "./file-field-view.ts";

type Edited = ReturnType<typeof edited>;

/**
 * Sends the files chosen in `controls`, then saves the document `values` reads with them; a save that does not happen
 * deletes the files it sent, so nothing the record does not name is left, and the form holds the chosen files again.
 */
export async function saveWithFiles(controls: readonly Pick<Control, "settle" | "undo">[], attach: Attach | null, values: () => Edited, save: (made: RecordDocument) => Promise<string | null>): Promise<string | null> {
  const sent: string[] = [];
  const undo = async (): Promise<void> => {
    await Promise.all(sent.map((id) => attach?.discard(id)));
    controls.forEach((made) => made.undo?.());
  };
  for (const made of controls) {
    const settled = (await made.settle?.()) ?? { ok: true, sent: null };
    if (!settled.ok) {
      await undo();
      return settled.words;
    }
    if (settled.sent !== null) {
      sent.push(settled.sent);
    }
  }
  const result = values();
  const refused = result.ok ? await save(result.document) : "Some values can't be kept. Each one says why below its label.";
  if (refused !== null) {
    await undo();
  }
  return refused;
}
