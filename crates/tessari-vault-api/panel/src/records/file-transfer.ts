// Moving a sealed file to and from the server in parts. No DOM here, so it is tested.

/** The server's ceiling on one part, written or read. */
export const PART_BYTES = 4 * 1024 * 1024;

/** The parts of a `total`-byte sealed file, in order. */
export function partsOf(total: number): ReadonlyArray<{ readonly offset: number; readonly length: number }> {
  return Array.from({ length: Math.ceil(total / PART_BYTES) }, (_, index) => ({
    offset: index * PART_BYTES,
    length: Math.min(PART_BYTES, total - index * PART_BYTES),
  }));
}
