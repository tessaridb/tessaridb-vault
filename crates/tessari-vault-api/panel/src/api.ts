// The one way the page talks to the server: same-origin JSON under /api, the session's bearer token on every call that
// has one. Every answer is checked against its reader before it is used, so nothing the server sends is trusted for
// its shape.

import { bearer } from "./session.ts";

export type Failure = { readonly ok: false; readonly status: number; readonly code: string; readonly message: string };
export type Answer<T> = { readonly ok: true; readonly value: T } | Failure;

export type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

/** What a call may add: a JSON body, and the reason a person gave for a change (recorded on the trail). */
export interface Options {
  readonly body?: Readonly<Record<string, unknown>>;
  readonly reason?: string;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** A refusal body as the server writes it: `{ error_code, message }`. */
export function readRefusal(v: unknown): { readonly code: string; readonly message: string } | null {
  if (!isRecord(v)) {
    return null;
  }
  const code = v["error_code"];
  const message = v["message"];
  return typeof code === "string" && typeof message === "string" ? { code, message } : null;
}

/** For answers whose body is not read (204). */
export const ignored = (_v: unknown): true => true;

function headers(options: Options): Record<string, string> {
  const made: Record<string, string> = { accept: "application/json" };
  if (options.body !== undefined) {
    made["content-type"] = "application/json";
  }
  const token = bearer();
  if (token !== null) {
    made["authorization"] = `Bearer ${token}`;
  }
  if (options.reason !== undefined && options.reason !== "") {
    // A header carries ASCII only; the server decodes the reason back to the text the person wrote.
    made["vault-reason"] = encodeURIComponent(options.reason);
  }
  return made;
}

/** Calls `method route` and reads the answer with `read`; a network failure and an unreadable answer are failures too. */
export async function call<T>(method: Method, route: string, read: (value: unknown) => T | null, options: Options = {}): Promise<Answer<T>> {
  let response: Response;
  try {
    response = await fetch(route, {
      method,
      credentials: "same-origin",
      headers: headers(options),
      ...(options.body === undefined ? {} : { body: JSON.stringify(options.body) }),
    });
  } catch {
    return { ok: false, status: 0, code: "network", message: "The vault did not answer." };
  }
  // No body (a 204, or a bare 201 for a write that answers nothing) reads as `null`, which `ignored` takes and every
  // other reader refuses; a body that is not JSON is unreadable.
  let parsed: unknown = null;
  const text = response.status === 204 ? "" : await response.text().catch(() => undefined);
  if (text === undefined) {
    parsed = undefined;
  } else if (text !== "") {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = undefined;
    }
  }
  if (response.ok) {
    const value = parsed === undefined ? null : read(parsed);
    return value === null
      ? { ok: false, status: response.status, code: "bad_answer", message: "The vault answered in a shape this page does not read." }
      : { ok: true, value };
  }
  const refusal = readRefusal(parsed);
  return {
    ok: false,
    status: response.status,
    code: refusal?.code ?? "unknown",
    message: refusal?.message ?? `The vault refused the request (status ${response.status}).`,
  };
}
