// Where a record page sits under Records, as the trail drawn above its heading. Every step but the page itself links
// back, so the edit page returns to its record and a chosen type returns to the choice of type.

/** One step of the trail; the page itself has no link. */
export interface Crumb {
  readonly label: string;
  readonly href: string | null;
}

const RECORDS = "Records";

const named = (title: string | null): string => (title === null ? "Record" : title === "" ? "Untitled" : title);

/** The trail of a record's own page; `title` is null until the record is open. */
export const recordCrumbs = (records: string, title: string | null): Crumb[] => [
  { label: RECORDS, href: records },
  { label: named(title), href: null },
];

/** The trail of a record's edit page, whose record links back to the record. */
export const editCrumbs = (records: string, record: string, title: string | null): Crumb[] => [
  { label: RECORDS, href: records },
  { label: named(title), href: record },
  { label: "Edit", href: null },
];

/** The trail of the new-record page, before and after a type is chosen. */
export function newCrumbs(records: string, chooser: string, typeName: string | null): Crumb[] {
  const top = { label: RECORDS, href: records };
  return typeName === null ? [top, { label: "New record", href: null }] : [top, { label: "New record", href: chooser }, { label: typeName, href: null }];
}
