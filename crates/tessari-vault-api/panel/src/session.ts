// The session's bearer token, kept in this page's memory and nowhere else: not in localStorage, sessionStorage or a
// cookie the page can read. A reload therefore signs the person out, which costs little: the unlocked keys live in
// memory too, so a reload needs the unlock password again anyway.

let token: string | null = null;

/** The current session's token, or null when nobody is signed in. */
export const bearer = (): string | null => token;

/** Keeps `value` as the session token; an empty value is no session. */
export function keep(value: string): void {
  token = value === "" ? null : value;
}

/** Forgets the session, as after signing out or a refusal saying it ended. */
export function forget(): void {
  token = null;
}
