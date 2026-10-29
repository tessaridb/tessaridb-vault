// The words for a refused delete, kept apart from the dialog so they can be checked without a page.

/** What to say when deleting a record answered `status` and `code`; `fallback` covers everything else. */
export function deleteFailure(status: number, code: string, fallback: string): string {
  if (status === 403) {
    return "Only the record's author or a company admin can delete it.";
  }
  if (status === 404) {
    return "This record is already gone, or it isn't shared with you.";
  }
  if (status === 409 && code === "stale_revision") {
    return "This record changed while you were deciding. Open it again, then delete it.";
  }
  return fallback;
}
