// The company trail besides its page: the events and the chain check it reads, each action the server records in
// words, the area an action belongs to for the filter, who and what an event names, and when another page is there.
// No DOM here, so all of it is tested.

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** One event of the trail: done by `actor`, at `at` seconds since 1970, to `target`, for `reason` when one was given. */
export interface TrailEvent {
  readonly seq: number;
  readonly at: number;
  readonly actor: string;
  readonly action: string;
  readonly target: string;
  readonly reason: string;
}

/** Whether the trail's chain holds, and the first event that breaks it. */
export interface Verified {
  readonly intact: boolean;
  readonly brokenAt: number | null;
}

/** Where an action belongs, for the trail's filter. */
export type Area = "records" | "members" | "groups" | "keys" | "company" | "other";

/** The answer of `GET …/audit`. */
export function readEvents(v: unknown): readonly TrailEvent[] | null {
  if (!isRecord(v) || !Array.isArray(v["events"])) {
    return null;
  }
  const events = v["events"].map((each: unknown): TrailEvent | null => {
    if (!isRecord(each)) {
      return null;
    }
    const { seq, at, actor, action, target, reason } = each;
    return typeof seq === "number" && typeof at === "number" && typeof actor === "string" && typeof action === "string" && typeof target === "string" && typeof reason === "string"
      ? { seq, at, actor, action, target, reason }
      : null;
  });
  return events.every((each) => each !== null) ? events : null;
}

/** The answer of `GET …/audit/verify`. */
export function readVerified(v: unknown): Verified | null {
  if (!isRecord(v) || typeof v["intact"] !== "boolean") {
    return null;
  }
  const broken = v["broken_at"];
  return typeof broken === "number" || broken === null || broken === undefined ? { intact: v["intact"], brokenAt: typeof broken === "number" ? broken : null } : null;
}

const WORDS: ReadonlyMap<string, string> = new Map([
  ["chain.append", "Vouched for a key"],
  ["company.create", "Created the company"],
  ["company.resume", "Resumed the company"],
  ["company.suspend", "Suspended the company"],
  ["enrolment.issue", "Issued a join link"],
  ["file.complete", "Finished uploading a file"],
  ["file.create", "Started uploading a file"],
  ["file.read", "Downloaded a file"],
  ["group_key.keep", "Keyed a group"],
  ["group_key.read", "Opened a group's key"],
  ["group_key.rotate", "Made a group a new key"],
  ["group_key.wrap", "Gave a group's key to its members"],
  ["group.create", "Created a group"],
  ["group.delete", "Deleted a group"],
  ["group.member.add", "Added someone to a group"],
  ["group.member.remove", "Took someone out of a group"],
  ["group.rename", "Renamed a group"],
  ["keys.escrow", "Escrowed their keys for recovery"],
  ["keys.keep", "Set up their keys"],
  ["keys.read", "Opened their keys"],
  ["keys.rewrap", "Set a new unlock password"],
  ["member.add", "Added a member"],
  ["member.change", "Changed a member"],
  ["member.escrow.read", "Read a member's escrowed keys"],
  ["member.recover", "Handed a member's keys over"],
  ["member.recovery.redeem", "Took their recovered keys back"],
  ["member.recovery.replay", "Tried a used recovery again"],
  ["member.recovery.request", "Asked for recovery"],
  ["member.remove", "Removed a member"],
  ["record_type.define", "Defined a record type"],
  ["record.create", "Created a record"],
  ["record.delete", "Deleted a record"],
  ["record.read", "Opened a record"],
  ["record.revise", "Changed a record"],
  ["record.share", "Shared a record"],
  ["record.unshare", "Removed a share"],
  ["recovery.key.read", "Read the recovery key"],
  ["recovery.key", "Kept the recovery key"],
]);

/** `action` in words; an action this page does not know is shown as the server wrote it, never hidden. */
export const actionWords = (action: string): string => WORDS.get(action) ?? action;

/** The area `action` belongs to. */
export function areaOf(action: string): Area {
  const head = action.split(".")[0] ?? "";
  switch (head) {
    case "record":
    case "record_type":
    case "file":
      return "records";
    case "member":
    case "enrolment":
      return "members";
    case "group":
    case "group_key":
      return "groups";
    case "keys":
    case "chain":
    case "recovery":
      return "keys";
    case "company":
      return "company";
    default:
      return "other";
  }
}

/** Who did an event: the person's name when `names` knows them, else the start of their id. */
export const whoWords = (id: string, names: ReadonlyMap<string, string>): string => names.get(id) ?? id.slice(0, 8);

/** What an event was done to: a person or group by name, someone in a group as both, anything else as written. */
export function targetWords(target: string, names: ReadonlyMap<string, string>): string {
  const [group, account] = target.split(":");
  if (group !== undefined && account !== undefined && names.has(group) && names.has(account)) {
    return `${names.get(account)} in ${names.get(group)}`;
  }
  return names.get(target) ?? (target.length > 12 ? target.slice(0, 8) : target);
}

/** The chain check in words. */
export const verifyWords = (verified: Verified): string =>
  verified.intact
    ? "The trail is intact: no event was changed or removed."
    : `Event ${verified.brokenAt ?? "?"} doesn't check out: it, or one before it, was changed outside the vault. Tell your company's owner.`;

/** How many events the server sends at most in one page. */
const PAGE = 100;

/** Whether a page of `count` events came back full, so older ones may be there. */
export const pageFull = (count: number): boolean => count >= PAGE;
