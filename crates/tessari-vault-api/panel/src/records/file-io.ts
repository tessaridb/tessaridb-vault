// Attaching a file to a saved record, deleting one a save then failed to name, and reading one back. The file is sealed in this page under a key of its own, made
// here; the server is told the sealed size, answers the file's id, and takes the sealed bytes in parts, in order. Reading
// fetches the parts and opens them with the key the record's file field carries.

import { call, callBytes, ignored } from "../api.ts";
import { concat, type Bytes } from "../crypto/bytes.ts";
import { VaultCryptoError } from "../crypto/errors.ts";
import { FILE_MAX_BYTES, fileKeyId, importFileKey, newFileKey, openFile, sealFile, sealedSize } from "../crypto/files.ts";
import type { Unlocked } from "../vault.ts";
import type { FileValue } from "./fields.ts";
import { partsOf } from "./file-transfer.ts";
import { loadFailure } from "./list-view.ts";
import { companyPath } from "./load.ts";

type Outcome<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly words: string };

const readCreated = (v: unknown): string | null =>
  typeof v === "object" && v !== null && "id" in v && typeof v.id === "string" && /^[0-9a-f]{32}$/.test(v.id) ? v.id : null;

const filesPath = (held: Unlocked, record: string): string => `${companyPath(held)}/records/${encodeURIComponent(record)}/files`;

/** Seals `file` and sends it to record `record`; the answer is the value the record's file field keeps. */
export async function uploadFile(held: Unlocked, record: string, file: File): Promise<Outcome<FileValue>> {
  if (file.size > FILE_MAX_BYTES) {
    return { ok: false, words: "A file can be up to 256 MiB." };
  }
  const data = new Uint8Array(await file.arrayBuffer());
  const own = newFileKey();
  const created = await call("POST", filesPath(held, record), readCreated, { body: { size: sealedSize(data.length) } });
  if (!created.ok) {
    return { ok: false, words: loadFailure(created) };
  }
  const context = { company: held.company, record, file: created.value, keyId: fileKeyId(own.keyId) };
  const sealed = await sealFile(await importFileKey(own.key), context, data);
  for (const part of partsOf(sealed.length)) {
    const sent = await callBytes("PUT", `${filesPath(held, record)}/${created.value}?offset=${part.offset}`, sealed.slice(part.offset, part.offset + part.length));
    if (!sent.ok) {
      return { ok: false, words: loadFailure(sent) };
    }
  }
  return { ok: true, value: { id: created.value, name: file.name, size: data.length, mediaType: file.type === "" ? "application/octet-stream" : file.type, ...own } };
}

/** Deletes the file `id` this page sent to record `record` when the save that was to name it did not happen; the vault
 * keeps no file nobody names, and one left behind by a failed delete stays sealed under a key nobody kept. */
export async function deleteFile(held: Unlocked, record: string, id: string): Promise<void> {
  await call("DELETE", `${filesPath(held, record)}/${encodeURIComponent(id)}`, ignored);
}

/** Reads the file `value` names on record `record` and opens it; a file that does not check out is refused whole. */
export async function downloadFile(held: Unlocked, record: string, value: FileValue): Promise<Outcome<Bytes>> {
  const parts: Bytes[] = [];
  for (const part of partsOf(sealedSize(value.size))) {
    const read = await callBytes("GET", `${filesPath(held, record)}/${encodeURIComponent(value.id)}?offset=${part.offset}&limit=${part.length}`);
    if (!read.ok) {
      return { ok: false, words: loadFailure(read) };
    }
    parts.push(read.value);
  }
  const context = { company: held.company, record, file: value.id, keyId: fileKeyId(value.keyId) };
  try {
    return { ok: true, value: await openFile(new Map([[value.keyId, await importFileKey(value.key)]]), context, concat(...parts)) };
  } catch (problem) {
    if (problem instanceof VaultCryptoError) {
      return { ok: false, words: "This file doesn't check out, so it isn't opened. It may have been changed on the server." };
    }
    throw problem;
  }
}
