"use strict";
(() => {
  // src/session.ts
  var token = null;
  var bearer = () => token;
  function keep(value) {
    token = value === "" ? null : value;
  }
  function forget() {
    token = null;
  }

  // src/api.ts
  var isRecord = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  function readRefusal(v) {
    if (!isRecord(v)) {
      return null;
    }
    const code = v["error_code"];
    const message = v["message"];
    return typeof code === "string" && typeof message === "string" ? { code, message } : null;
  }
  var ignored = (_v) => true;
  function headers(options) {
    const made = { accept: "application/json" };
    if (options.body !== void 0) {
      made["content-type"] = "application/json";
    }
    const token2 = bearer();
    if (token2 !== null) {
      made["authorization"] = `Bearer ${token2}`;
    }
    if (options.reason !== void 0 && options.reason !== "") {
      made["vault-reason"] = options.reason;
    }
    return made;
  }
  async function call(method, route, read, options = {}) {
    let response;
    try {
      response = await fetch(route, {
        method,
        credentials: "same-origin",
        headers: headers(options),
        ...options.body === void 0 ? {} : { body: JSON.stringify(options.body) }
      });
    } catch {
      return { ok: false, status: 0, code: "network", message: "The vault did not answer." };
    }
    let parsed = null;
    if (response.status !== 204) {
      try {
        parsed = await response.json();
      } catch {
        parsed = void 0;
      }
    }
    if (response.ok) {
      const value = parsed === void 0 ? null : read(parsed);
      return value === null ? { ok: false, status: response.status, code: "bad_answer", message: "The vault answered in a shape this page does not read." } : { ok: true, value };
    }
    const refusal2 = readRefusal(parsed);
    return {
      ok: false,
      status: response.status,
      code: refusal2?.code ?? "unknown",
      message: refusal2?.message ?? `The vault refused the request (status ${response.status}).`
    };
  }

  // src/guards.ts
  var SIGN_IN = "/sign-in";
  function needsSession(signedIn2, current) {
    return () => signedIn2() ? { kind: "allow" } : { kind: "redirect", to: `#${SIGN_IN}?next=${encodeURIComponent(current().replace(/^#/, ""))}` };
  }
  function afterSignIn(next) {
    if (next === null || !next.startsWith("/") || next.startsWith("//") || next.startsWith(SIGN_IN)) {
      return "#/";
    }
    return `#${next}`;
  }

  // src/router/history.ts
  function browserHistory(window2) {
    return {
      current: () => window2.location.hash || "#/",
      push: (href) => window2.history.pushState(null, "", href),
      replace: (href) => window2.history.replaceState(null, "", href),
      subscribe: (listener) => {
        window2.addEventListener("popstate", listener);
        return () => window2.removeEventListener("popstate", listener);
      }
    };
  }

  // src/router/pattern.ts
  function segments(path) {
    return path.split("/").filter((segment, index) => index > 0 || segment !== "");
  }
  function build(pattern, params) {
    const filled = segments(pattern).map(
      (segment) => segment.startsWith(":") ? encodeURIComponent(params[segment.slice(1)] ?? "") : segment
    );
    return `/${filled.join("/")}`;
  }
  function match(pattern, path) {
    const wanted = segments(pattern);
    const given = segments(path);
    if (wanted.length !== given.length) {
      return null;
    }
    const params = {};
    for (const [index, segment] of wanted.entries()) {
      const value = given[index] ?? "";
      if (segment.startsWith(":")) {
        if (value === "") {
          return null;
        }
        try {
          params[segment.slice(1)] = decodeURIComponent(value);
        } catch {
          return null;
        }
      } else if (segment !== value) {
        return null;
      }
    }
    return params;
  }

  // src/router/query.ts
  var text = {
    parse: (raw) => raw,
    write: (value) => typeof value === "string" ? value : null
  };
  function readQuery(spec, raw) {
    const given = new URLSearchParams(raw);
    const values = {};
    for (const [key, codec] of Object.entries(spec ?? {})) {
      const value = given.get(key);
      values[key] = value === null ? null : codec.parse(value);
    }
    return values;
  }
  function writeQuery(spec, values) {
    const pairs = [];
    for (const [key, codec] of Object.entries(spec ?? {})) {
      const written = codec.write(values[key]);
      if (written !== null) {
        pairs.push(`${key}=${encodeURIComponent(written)}`);
      }
    }
    return pairs.length === 0 ? "" : `?${pairs.join("&")}`;
  }

  // src/router/router.ts
  function defineRoutes(routes2) {
    return routes2;
  }
  var REDIRECT_CEILING = 8;
  function split(href) {
    const bare = href.startsWith("#") ? href.slice(1) : href;
    const at = bare.indexOf("?");
    const path = at === -1 ? bare : bare.slice(0, at);
    return { path: path === "" ? "/" : path, query: at === -1 ? "" : bare.slice(at + 1) };
  }
  var Router = class {
    #routes;
    #history;
    #listener;
    constructor(routes2, history2) {
      this.#routes = routes2;
      this.#history = history2;
    }
    /** The link to route `name`. */
    href(name, params, query) {
      const spec = this.#routes[name];
      if (spec === void 0) {
        throw new RangeError(`no route named ${name}`);
      }
      return `#${build(spec.path, params)}${writeQuery(spec.query, query ?? {})}`;
    }
    /** The route `href` names, or `null` when it names none. */
    resolve(href) {
      const { path, query } = split(href);
      for (const [name, spec] of Object.entries(this.#routes)) {
        const params = match(spec.path, path);
        if (params !== null) {
          const resolved = { name, params, query: readQuery(spec.query, query) };
          if (this.#isMatch(resolved)) {
            return resolved;
          }
        }
      }
      return null;
    }
    /** Answer the current URL now and every later one, through `listener`; the returned call stops listening. */
    start(listener) {
      this.#listener = listener;
      const stop = this.#history.subscribe(() => this.#settle());
      this.#settle();
      return () => {
        stop();
        this.#listener = void 0;
      };
    }
    /** Go to route `name`, as a link does. */
    navigate(name, params, query) {
      this.#history.push(this.href(name, params, query));
      this.#settle();
    }
    /** Resolve the current URL, following its guards' redirects up to the ceiling, and report what it came to. */
    #settle() {
      let href = this.#history.current();
      for (let followed = 0; followed <= REDIRECT_CEILING; followed += 1) {
        const resolved = this.resolve(href);
        if (resolved === null) {
          this.#listener?.({ kind: "not-found", href });
          return;
        }
        const verdict = this.#routes[resolved.name]?.guard?.() ?? { kind: "allow" };
        if (verdict.kind === "allow") {
          this.#listener?.({ kind: "matched", match: resolved });
          return;
        }
        this.#history.replace(verdict.to);
        href = verdict.to;
      }
      this.#listener?.({ kind: "loop", href });
    }
    /** Whether `value` is a match of a declared route: its parameters are exactly the ones its pattern names. */
    #isMatch(value) {
      const spec = this.#routes[value.name];
      if (spec === void 0) {
        return false;
      }
      const names = spec.path.split("/").filter((segment) => segment.startsWith(":")).map((segment) => segment.slice(1));
      const given = Object.keys(value.params);
      return names.length === given.length && names.every((name) => given.includes(name));
    }
  };

  // src/base64url.ts
  function encode(bytes) {
    return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join("")).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
  }
  function decode(text2) {
    if (!/^[A-Za-z0-9_-]*$/.test(text2) || text2.length % 4 === 1) {
      return null;
    }
    const standard = text2.replaceAll("-", "+").replaceAll("_", "/");
    return Uint8Array.from(atob(standard.padEnd(Math.ceil(standard.length / 4) * 4, "=")), (character) => character.charCodeAt(0));
  }

  // src/dom.ts
  function el(tag, attributes = {}, ...children) {
    const made = document.createElement(tag);
    for (const [name, value] of Object.entries(attributes)) {
      made.setAttribute(name, value);
    }
    for (const child of children) {
      if (child !== null && child !== false) {
        made.append(child);
      }
    }
    return made;
  }
  function fill(parent, ...children) {
    parent.replaceChildren(...children.filter((child) => child !== null && child !== false));
  }
  function field(id, label, attributes = {}, hint) {
    const input = el("input", { id, name: id, type: "text", ...attributes });
    const described = hint === void 0 ? null : el("p", { id: `${id}-hint`, class: "hint" }, hint);
    if (described !== null) {
      input.setAttribute("aria-describedby", described.id);
    }
    return { row: el("div", { class: "field" }, el("label", { for: id }, label), input, described), input };
  }

  // src/screen.ts
  var heading = (title) => el("h1", { tabindex: "-1", class: "view-title" }, title);
  var refusal = (words) => el("p", { class: "message bad", role: "alert" }, words);

  // src/sign-in-model.ts
  var isRecord2 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  function readMethods(v) {
    if (!isRecord2(v)) {
      return null;
    }
    const { password, passkey: passkey2, provider } = v;
    return typeof password === "boolean" && typeof passkey2 === "boolean" && typeof provider === "boolean" ? { password, passkey: passkey2, provider } : null;
  }
  function readChallenge(v) {
    return isRecord2(v) && typeof v["challenge"] === "string" && v["challenge"] !== "" ? v["challenge"] : null;
  }
  function readSession(v) {
    return isRecord2(v) && typeof v["session"] === "string" && v["session"] !== "" ? v["session"] : null;
  }
  function readPasskeyRequest(v) {
    return isRecord2(v) && typeof v["challenge"] === "string" && typeof v["rp_id"] === "string" ? { challenge: v["challenge"], rpId: v["rp_id"] } : null;
  }
  function signInFailure(failure) {
    switch (failure.code) {
      case "sign_in_failed":
        return "That email and password don't open an account here. Check both and try again.";
      case "wrong_code":
        return "That code didn't match. Use the newest code from your authenticator app.";
      case "passkey_refused":
        return "That passkey wasn't accepted here. Try again, or sign in with your password.";
      case "throttled":
        return "Too many tries from here. Wait a few minutes, then try again.";
      case "network":
        return "The vault didn't answer. Check your connection, then try again.";
      case "unavailable":
        return "The vault can't sign you in right now. Try again in a minute.";
      default:
        return failure.status >= 500 ? "The vault couldn't finish signing you in. Try again; if it keeps failing, tell your admin." : "The vault refused this sign-in. Try again.";
    }
  }
  var isProviderReturn = (search) => {
    const query = new URLSearchParams(search);
    return query.has("state") && (query.has("code") || query.has("error"));
  };
  var PROVIDER_START = "/api/sign-in/oidc";

  // src/sign-in.ts
  function card(title, lead, ...body) {
    return el(
      "div",
      { class: "gate" },
      el(
        "form",
        { class: "card", novalidate: "" },
        el("img", { class: "logo", src: "/favicon.svg", alt: "", width: "48", height: "48" }),
        heading(title),
        el("p", { class: "muted" }, lead),
        ...body
      )
    );
  }
  async function passkey(request) {
    const challenge = decode(request.challenge);
    if (challenge === null) {
      return { ok: false, status: 0, code: "bad_answer", message: "" };
    }
    let credential;
    try {
      credential = await navigator.credentials.get({
        publicKey: { challenge, rpId: request.rpId, userVerification: "required" }
      });
    } catch {
      return { ok: false, status: 0, code: "passkey_refused", message: "" };
    }
    if (!(credential instanceof PublicKeyCredential) || !(credential.response instanceof AuthenticatorAssertionResponse)) {
      return { ok: false, status: 0, code: "passkey_refused", message: "" };
    }
    const response = credential.response;
    if (response.userHandle === null) {
      return { ok: false, status: 0, code: "passkey_refused", message: "" };
    }
    const answer = await call("POST", "/api/sign-in/passkey", readSession, {
      body: {
        credential_id: encode(new Uint8Array(credential.rawId)),
        client_data_json: encode(new Uint8Array(response.clientDataJSON)),
        authenticator_data: encode(new Uint8Array(response.authenticatorData)),
        signature: encode(new Uint8Array(response.signature)),
        user_handle: encode(new Uint8Array(response.userHandle))
      }
    });
    return answer.ok ? answer.value : answer;
  }
  function codeStep(main, challenge, done, restart) {
    const code = field(
      "code",
      "Code from your authenticator app",
      { inputmode: "numeric", autocomplete: "one-time-code", pattern: "[0-9]{6}", maxlength: "6", required: "", spellcheck: "false" },
      "Six digits. A new one appears every 30 seconds."
    );
    const submit = el("button", { type: "submit", class: "primary" }, "Sign in");
    const other = el("button", { type: "button", class: "quiet" }, "Use a different account");
    const said = el("div", {});
    const view2 = card("Enter your code", "Your password was right. One more step.", code.row, submit, other, said);
    other.addEventListener("click", restart);
    view2.querySelector("form")?.addEventListener("submit", async (event) => {
      event.preventDefault();
      submit.disabled = true;
      const answer = await call("POST", "/api/sign-in/code", readSession, { body: { challenge, code: code.input.value.trim() } });
      code.input.value = "";
      submit.disabled = false;
      if (answer.ok) {
        keep(answer.value);
        done();
        return;
      }
      if (answer.code === "sign_in_failed") {
        fill(said, refusal("This sign-in expired. Start again with your password."));
        other.textContent = "Start again";
        other.focus();
        return;
      }
      code.input.setAttribute("aria-invalid", "true");
      fill(said, refusal(signInFailure(answer)));
      code.input.focus();
    });
    fill(main, view2);
    code.input.focus();
  }
  function signIn(main, done, notice2 = null) {
    const restart = () => signIn(main, done);
    const email = field("email", "Work email", { type: "email", autocomplete: "username", required: "", spellcheck: "false" });
    const password = field("password", "Password", { type: "password", autocomplete: "current-password", required: "" });
    const submit = el("button", { type: "submit", class: "primary" }, "Continue");
    const others = el("div", { class: "other-ways" });
    const said = el("div", {}, notice2 === null ? null : refusal(notice2));
    const view2 = card(
      "Sign in to TessariDB Vault",
      "Use your work email and password, then a code from your authenticator app.",
      email.row,
      password.row,
      submit,
      others,
      said
    );
    view2.querySelector("form")?.addEventListener("submit", async (event) => {
      event.preventDefault();
      submit.disabled = true;
      const answer = await call("POST", "/api/sign-in", readChallenge, {
        body: { email: email.input.value.trim(), password: password.input.value }
      });
      password.input.value = "";
      submit.disabled = false;
      if (answer.ok) {
        codeStep(main, answer.value, done, restart);
        return;
      }
      fill(said, refusal(signInFailure(answer)));
      password.input.focus();
    });
    fill(main, view2);
    email.input.focus();
    void call("GET", "/api/sign-in/methods", readMethods).then((methods) => {
      if (!methods.ok) {
        return;
      }
      const offered = [];
      if (methods.value.passkey && typeof PublicKeyCredential === "function") {
        const button = el("button", { type: "button" }, "Sign in with a passkey");
        button.addEventListener("click", async () => {
          button.disabled = true;
          const begun = await call("POST", "/api/sign-in/passkey/begin", readPasskeyRequest);
          const result = begun.ok ? await passkey(begun.value) : begun;
          button.disabled = false;
          if (typeof result === "string") {
            keep(result);
            done();
            return;
          }
          fill(said, refusal(signInFailure(result)));
        });
        offered.push(button);
      }
      if (methods.value.provider) {
        offered.push(el("a", { class: "button", href: PROVIDER_START }, "Sign in with your organisation"));
      }
      if (offered.length > 0) {
        fill(others, el("p", { class: "muted" }, "or"), ...offered);
      }
    });
  }
  async function providerReturned() {
    const search = window.location.search;
    if (!isProviderReturn(search)) {
      return null;
    }
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.hash}`);
    const answer = await call("GET", `/api/sign-in/oidc/return${search}`, readSession);
    if (answer.ok) {
      keep(answer.value);
      return null;
    }
    return signInFailure(answer);
  }

  // src/app.ts
  var history = browserHistory(window);
  var notice = null;
  var signedIn = () => bearer() !== null;
  var session = needsSession(signedIn, () => history.current());
  var routes = defineRoutes({
    records: { path: "/", guard: session },
    signIn: { path: "/sign-in", query: { next: text } }
  });
  function heading2(view2, title, lead) {
    const h1 = document.createElement("h1");
    h1.textContent = title;
    const p = document.createElement("p");
    p.textContent = lead;
    view2.replaceChildren(h1, p);
  }
  var go = (href) => {
    window.location.hash = href;
  };
  function draw(outcome, view2) {
    document.body.classList.toggle("signed-out", !signedIn());
    if (outcome.kind !== "matched") {
      heading2(view2, "Not found", "This address names no page of the vault.");
      return;
    }
    switch (outcome.match.name) {
      case "records":
        heading2(view2, "Records", "Your company's records arrive with the next version of this page.");
        break;
      case "signIn": {
        if (signedIn()) {
          go(afterSignIn(outcome.match.query.next));
          return;
        }
        const next = outcome.match.query.next;
        signIn(view2, () => go(afterSignIn(next)), notice);
        notice = null;
        break;
      }
    }
  }
  function mark(outcome) {
    const section = outcome.kind === "matched" ? outcome.match.name : "";
    for (const link of document.querySelectorAll("nav a[data-section]")) {
      if (link.dataset.section === section) {
        link.setAttribute("aria-current", "page");
      } else {
        link.removeAttribute("aria-current");
      }
    }
  }
  function themed() {
    const button = document.getElementById("theme");
    const root = document.documentElement;
    const label = () => {
      if (button !== null) {
        button.textContent = root.dataset.theme === "dark" ? "Light theme" : "Dark theme";
      }
    };
    const stored = localStorage.getItem("vault-theme");
    if (stored === "dark" || stored === "light") {
      root.dataset.theme = stored;
    }
    label();
    button?.addEventListener("click", () => {
      root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark";
      localStorage.setItem("vault-theme", root.dataset.theme);
      label();
    });
  }
  function signOutButton() {
    document.getElementById("sign-out")?.addEventListener("click", async () => {
      await call("DELETE", "/api/session", ignored);
      forget();
      go("#/sign-in");
    });
  }
  var view = document.getElementById("view");
  if (view !== null) {
    themed();
    signOutButton();
    void providerReturned().then((refused) => {
      notice = refused;
      new Router(routes, history).start((outcome) => {
        draw(outcome, view);
        mark(outcome);
      });
    });
    document.getElementById("skip")?.addEventListener("click", (event) => {
      event.preventDefault();
      view.focus();
    });
  }
})();
