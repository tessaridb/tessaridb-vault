// Asking for recovery, from the unlock card of someone who forgot their unlock password. Their browser makes a one-time
// key whose private half never leaves this page, asks with its public half, and shows a code to read to the owner who
// hands the keys over. Redeeming takes the keys back once; they are then wrapped under a new unlock password, with the
// same key set and the same pinned owner key, so nothing the person could open before is lost.

import { call, ignored } from "../api.ts";
import { handoffCode } from "../crypto/fingerprint.ts";
import { restoreKeySet, type OpenKeySet, type StoredKeySet } from "../crypto/key-set.ts";
import { generatePair, publicBytes } from "../crypto/keys.ts";
import { unwrapSecret } from "../crypto/wraps.ts";
import { el, field, fill } from "../dom.ts";
import { refusal } from "../screen.ts";
import { keyIdBytes, newPasswordProblem, type Company, type Me } from "../unlock-model.ts";
import { askBody, readAsked, readHandoff, recoveryFailure, rewrapBody } from "./recovery-model.ts";

/** What the unlock page lends the recovery cards. */
export interface AskContext {
  readonly me: Me;
  readonly company: Company;
  readonly stored: StoredKeySet;
  readonly card: (title: string, lead: string, ...body: Array<HTMLElement | null>) => HTMLElement;
  readonly module: () => Promise<WebAssembly.Module>;
  /** Back to the unlock form. */
  readonly back: () => void;
  /** The keys are open again under the new unlock password. */
  readonly opened: (keys: OpenKeySet) => void;
}

/** A request this page asked: its id, and the one-time key the handoff is wrapped to. */
interface Asked {
  readonly id: string;
  readonly handoff: CryptoKeyPair;
  readonly handoffPublic: Uint8Array<ArrayBuffer>;
}

const path = (context: AskContext): string => `/api/companies/${encodeURIComponent(context.company.id)}`;

/** The first card: what recovery is, and asking. */
export function askForRecovery(main: HTMLElement, context: AskContext): void {
  const ask = el("button", { type: "button", class: "primary" }, "Ask an owner");
  const back = el("button", { type: "button", class: "quiet" }, "Back to unlock");
  const said = el("div", { "aria-live": "polite" });
  const lead = `An owner of ${context.company.name} can hand your keys back to you. They see that you asked, and you see that they did.`;
  fill(main, context.card("Forgot your unlock password?", lead, said, ask, back));
  back.addEventListener("click", context.back);
  ask.addEventListener("click", () => {
    ask.disabled = true;
    void (async () => {
      const handoff = await generatePair("X25519", false);
      const handoffPublic = await publicBytes(handoff.publicKey);
      const answer = await call("POST", `${path(context)}/recovery-requests`, readAsked, { body: askBody(handoffPublic) });
      ask.disabled = false;
      if (!answer.ok) {
        fill(said, refusal(recoveryFailure(answer)));
        return;
      }
      await waiting(main, context, { id: answer.value, handoff, handoffPublic });
    })();
  });
  ask.focus();
}

/** The second card: the code to read out, and checking whether an owner has handed the keys over. */
async function waiting(main: HTMLElement, context: AskContext, asked: Asked): Promise<void> {
  const check = el("button", { type: "button", class: "primary" }, "Check");
  const said = el("div", { "aria-live": "polite" });
  const lead = `Tell an owner of ${context.company.name} that you asked. When they hand your keys over, read them this code:`;
  const view = context.card(
    "Waiting for an owner",
    lead,
    el("p", { class: "fingerprint" }, await handoffCode(asked.handoffPublic)),
    el("p", { class: "muted" }, "Keep this tab open until they have. If you close it, ask again."),
    said,
    check,
  );
  fill(main, view);
  check.addEventListener("click", () => {
    check.disabled = true;
    void call("POST", `${path(context)}/recovery-requests/${encodeURIComponent(asked.id)}/redeem`, readHandoff).then(async (answer) => {
      check.disabled = false;
      if (!answer.ok) {
        fill(said, refusal(recoveryFailure(answer)));
        return;
      }
      const requestId = keyIdBytes(asked.id);
      try {
        if (requestId === null) {
          throw new RangeError("request id");
        }
        const wrapContext = { company: context.company.id, wrappedKind: "member-recovery", wrappedKeyId: context.stored.keyId, recipientKind: "member", recipientKeyId: requestId } as const;
        const escrow = await unwrapSecret(asked.handoff.privateKey, asked.handoffPublic, wrapContext, answer.value);
        newPassword(main, context, asked.id, escrow);
      } catch {
        fill(said, refusal("These keys didn't open on this page. Ask again."));
      }
    });
  });
  check.focus();
}

/** The third card: a new unlock password, then the keys wrapped under it and kept. */
function newPassword(main: HTMLElement, context: AskContext, request: string, escrow: Uint8Array<ArrayBuffer>): void {
  const password = field("new-unlock-password", "New unlock password", { type: "password", autocomplete: "new-password", required: "" }, "At least 12 characters. Not your sign-in password.");
  const repeated = field("repeat-unlock-password", "Type it again", { type: "password", autocomplete: "new-password", required: "" });
  const submit = el("button", { type: "submit", class: "primary" }, "Set unlock password");
  const said = el("div", { "aria-live": "polite" });
  const view = context.card("Set a new unlock password", "An owner handed your keys back. Set a new unlock password to keep them.", password.row, repeated.row, submit, said);
  let restored: { readonly stored: StoredKeySet; readonly open: OpenKeySet } | null = null;
  view.querySelector("form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const problem = newPasswordProblem(password.input.value, repeated.input.value, context.me.email);
    if (restored === null && problem !== null) {
      fill(said, refusal(problem));
      password.input.focus();
      return;
    }
    submit.disabled = true;
    fill(said, el("p", { class: "muted", role: "status" }, "Wrapping your keys. This takes a second."));
    void (async () => {
      try {
        restored ??= await restoreKeySet(await context.module(), password.input.value, context.me.account, context.company.id, context.stored, escrow);
      } catch {
        fill(said, refusal("These keys aren't the ones your account holds, so nothing was changed. Ask again."));
        return;
      } finally {
        password.input.value = "";
        repeated.input.value = "";
        submit.disabled = false;
      }
      const kept = await call("POST", `${path(context)}/keys/me/rewrap`, ignored, { body: rewrapBody(request, restored.stored) });
      if (kept.ok) {
        context.opened(restored.open);
        return;
      }
      fill(said, refusal(`${recoveryFailure(kept)} Your keys are still open on this page: select Set unlock password to try again.`));
    })();
  });
  fill(main, view);
  password.input.focus();
}
