// Which sidebar section a route belongs to: a page under a list marks that list, so the sidebar says where the person
// is from a record, a group or a member's page too. No DOM here, so it is tested.

const SECTIONS: ReadonlyMap<string, string> = new Map([
  ["records", "records"],
  ["record", "records"],
  ["newRecord", "records"],
  ["editRecord", "records"],
  ["groups", "groups"],
  ["group", "groups"],
  ["members", "members"],
  ["member", "members"],
  ["trail", "trail"],
  ["recovery", "recovery"],
  ["account", "account"],
  ["company", "company"],
]);

/** The `data-section` of the sidebar link for route `name`, or "" for a page outside the sections. */
export const sectionOf = (name: string): string => SECTIONS.get(name) ?? "";
