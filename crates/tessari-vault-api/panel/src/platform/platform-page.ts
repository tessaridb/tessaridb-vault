// The platform admin's Companies page: every company with its state, suspending and resuming one, creating one and
// handing its owner the join link, and confirming a deletion an owner asked for. Confirming is the second party to a
// deletion, so it asks for the company's exact name and a reason, and says what follows.

import { call, ignored } from "../api.ts";
import { announce, el, field, fill } from "../dom.ts";
import { deletionReady } from "../company/company-model.ts";
import { invitedBlock, type Invited } from "../members/add-member.ts";
import { joinLink } from "../members/member-admin.ts";
import { loadFailure } from "../records/list-view.ts";
import { heading, refusal } from "../screen.ts";
import { readCompanies } from "../unlock-model.ts";
import { platformFailure, platformRows, readCreated, readQueue, type Row } from "./platform-model.ts";

const STATE_WORDS = { active: "Active", suspended: "Suspended", closing: "Read-only, being deleted" } as const;

/** Draws the Companies page into `main`; `notice` is said once at the top, and `invited` is a new owner's join link, shown once. */
export async function drawPlatform(main: HTMLElement, notice: string | null = null, invited: Invited | null = null): Promise<void> {
  fill(main, el("div", { class: "page-head" }, heading("Companies")), el("p", { class: "muted", role: "status" }, "Loading the companies."));
  const [companies, queue] = await Promise.all([call("GET", "/api/companies", readCompanies), call("GET", "/api/company-drops", readQueue)]);
  if (!companies.ok || !queue.ok) {
    const failed = !companies.ok ? companies : queue.ok ? null : queue;
    fill(main, el("div", { class: "page-head" }, heading("Companies")), refusal(failed === null ? "" : failed.code === "forbidden" ? platformFailure(failed) : loadFailure(failed)));
    return;
  }
  const said = el("div", { "aria-live": "polite" }, notice === null ? null : el("p", { class: "muted", role: "status" }, notice));
  const again = (words: string | null): void => void drawPlatform(main, words);
  const rows = platformRows(companies.value, queue.value);
  const list =
    rows.length === 0
      ? el("p", { class: "muted" }, "No company yet. Create the first one below.")
      : el("ul", { class: "shares" }, ...rows.map((row) => companyRow(row, said, again)));
  fill(
    main,
    el("div", { class: "page-head" }, el("div", {}, heading("Companies"), el("p", { class: "lede" }, "Every company on this vault.")), el("a", { href: "#/unlock" }, "Unlock a company")),
    said,
    invited === null ? null : invitedBlock(invited),
    el("section", { class: "record-section", "aria-labelledby": "platform-list" }, el("h2", { id: "platform-list" }, "Companies"), list),
    createSection(said, (words, made) => void drawPlatform(main, words, made)),
  );
}

function companyRow(row: Row, said: HTMLElement, again: (words: string | null) => void): HTMLElement {
  const base = `/api/companies/${encodeURIComponent(row.id)}`;
  const send = (path: string, reason: string | undefined, words: string): void => {
    fill(said, el("p", { class: "muted", role: "status" }, "Saving."));
    void call("POST", path, ignored, reason === undefined ? {} : { reason }).then((answer) => (answer.ok ? again(words) : fill(said, refusal(platformFailure(answer)))));
  };
  const detail =
    row.deletion === null
      ? STATE_WORDS[row.state]
      : row.deletion.state === "requested"
        ? `${STATE_WORDS[row.state]} · an owner asked for its deletion`
        : `${STATE_WORDS[row.state]} · deleted after ${new Date(row.deletion.dropAfter * 1000).toLocaleString()}`;
  const actions: HTMLElement[] = [];
  if (row.state !== "closing") {
    const toggle = el("button", { type: "button" }, row.state === "active" ? "Suspend" : "Resume");
    toggle.addEventListener("click", () => send(`${base}/${row.state === "active" ? "suspend" : "resume"}`, undefined, row.state === "active" ? `${row.name} is suspended.` : `${row.name} is active again.`));
    actions.push(toggle);
  }
  if (row.deletion?.state === "requested") {
    actions.push(confirmControl(row, (reason) => send(`${base}/drop/confirm`, reason, `${row.name} is read-only and will be deleted in seven days unless an owner cancels.`)));
  }
  return el("li", { class: "share" }, el("div", {}, el("span", { class: "share-name" }, row.name), el("span", { class: "detail" }, detail)), el("span", { class: "row-actions" }, ...actions));
}

function confirmControl(row: Row, confirmed: (reason: string) => void): HTMLElement {
  const named = field(`confirm-name-${row.id}`, "Type the company's name to confirm", { autocomplete: "off" }, row.name);
  const why = field(`confirm-reason-${row.id}`, "Why you confirm it", { autocomplete: "off" }, "Kept on the company's trail.");
  const go = el("button", { type: "button", class: "danger", disabled: "" }, `Confirm deleting ${row.name}`);
  const keep = el("button", { type: "button", autofocus: "" }, "Not now");
  const dialog = el(
    "dialog",
    { class: "confirm", "aria-labelledby": `confirm-title-${row.id}` },
    el("h2", { id: `confirm-title-${row.id}` }, `Confirm deleting ${row.name}?`),
    el("p", {}, "An owner asked for this. Once you confirm, the company is read-only for seven days and any owner can still cancel; then the operator's deletion run deletes it for good, keeping its trail."),
    named.row,
    why.row,
    el("div", { class: "actions" }, keep, go),
  );
  const ready = (): void => {
    go.disabled = !deletionReady(named.input.value, why.input.value, row.name);
  };
  named.input.addEventListener("input", ready);
  why.input.addEventListener("input", ready);
  keep.addEventListener("click", () => dialog.close());
  go.addEventListener("click", () => {
    if (deletionReady(named.input.value, why.input.value, row.name)) {
      dialog.close();
      confirmed(why.input.value.trim());
    }
  });
  const open = el("button", { type: "button", class: "danger" }, "Confirm deletion");
  open.addEventListener("click", () => {
    named.input.value = "";
    why.input.value = "";
    ready();
    dialog.showModal();
  });
  return el("span", {}, open, dialog);
}

function createSection(said: HTMLElement, created: (words: string, invited: Invited | null) => void): HTMLElement {
  const name = field("new-company-name", "Company name", { autocomplete: "off" });
  const email = field("new-owner-email", "Owner's work email", { type: "email", autocomplete: "off" });
  const owner = field("new-owner-name", "Owner's name", { autocomplete: "off" });
  const create = el("button", { type: "submit" }, "Create company");
  const form = el("form", { novalidate: "" }, name.row, email.row, owner.row, create);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    create.disabled = true;
    const body = { name: name.input.value.trim(), owner_email: email.input.value.trim(), owner_name: owner.input.value.trim() };
    void call("POST", "/api/companies", readCreated, { body }).then((answer) => {
      create.disabled = false;
      if (!answer.ok) {
        fill(said, refusal(platformFailure(answer)));
        return;
      }
      announce(`${answer.value.name} is created.`);
      created(
        answer.value.token === null ? `${answer.value.name} is created. Its owner already has a sign-in and finds it after signing in.` : `${answer.value.name} is created. Send its owner the join link below.`,
        answer.value.token === null ? null : { name: body.owner_name, link: joinLink(window.location.origin, window.location.pathname, answer.value.token) },
      );
    });
  });
  return el("section", { class: "record-section", "aria-labelledby": "platform-create" }, el("h2", { id: "platform-create" }, "Create a company"), el("p", { class: "muted" }, "Its owner gets a single-use join link, shown here once. Send it to them yourself."), form);
}
