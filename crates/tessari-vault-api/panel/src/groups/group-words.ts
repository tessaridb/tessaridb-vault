// What the groups page's buttons say and what they are named. A button's name starts with the words it shows, so a
// person who says those words to their speech software reaches it; the group or person comes after, for a screen
// reader listing every button on the page.

/** A button's visible words and its accessible name. */
export interface ButtonWords {
  readonly text: string;
  readonly label: string;
}

/** The button that keys `group`, or makes it a new key when the old one is due. */
export function keyButton(group: string, rotate: boolean): ButtonWords {
  return rotate ? { text: "Make a new key", label: `Make a new key for ${group}` } : { text: "Key this group", label: `Key this group, ${group}` };
}

/** The button that gives `group`'s key to the `waiting` members who have none yet. */
export function giveButton(group: string, waiting: number): ButtonWords {
  return { text: `Give access (${waiting} waiting)`, label: `Give access (${waiting} waiting), ${group}` };
}

/** The button that opens `group`'s members. */
export function membersButton(group: string): ButtonWords {
  return { text: "Members", label: `Members of ${group}` };
}

/** The button that takes `person` out of `group`. */
export function takeOutButton(person: string, group: string): ButtonWords {
  return { text: "Take out", label: `Take out ${person} from ${group}` };
}
