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
  var UNLOCK = "/unlock";
  function gated(gate, open2, current) {
    return () => open2() ? { kind: "allow" } : { kind: "redirect", to: `#${gate}?next=${encodeURIComponent(current().replace(/^#/, ""))}` };
  }
  var needsSession = (signedIn2, current) => gated(SIGN_IN, signedIn2, current);
  var needsUnlock = (unlocked2, current) => gated(UNLOCK, unlocked2, current);
  function every(...guards) {
    return () => {
      for (const guard of guards) {
        const verdict = guard();
        if (verdict.kind !== "allow") {
          return verdict;
        }
      }
      return { kind: "allow" };
    };
  }
  function inPage(next, gates) {
    if (next === null || !next.startsWith("/") || next.startsWith("//") || gates.some((gate) => next.startsWith(gate))) {
      return "#/";
    }
    return `#${next}`;
  }
  var afterSignIn = (next) => inPage(next, [SIGN_IN]);
  var afterUnlock = (next) => inPage(next, [SIGN_IN, UNLOCK]);

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

  // src/crypto/errors.ts
  var VaultCryptoError = class extends Error {
    reason;
    constructor(reason) {
      super(`refused: ${reason}`);
      this.name = "VaultCryptoError";
      this.reason = reason;
    }
  };

  // src/crypto/bytes.ts
  var ENCODER = new TextEncoder();
  function utf8(text2) {
    return ENCODER.encode(text2);
  }
  function concat(...parts) {
    const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
    let offset = 0;
    for (const part of parts) {
      out.set(part, offset);
      offset += part.length;
    }
    return out;
  }
  function integer(value, width) {
    if (!Number.isSafeInteger(value) || value < 0 || width < 8 && value >= 2 ** (8 * width)) {
      throw new VaultCryptoError("malformed");
    }
    const out = new Uint8Array(width);
    let rest = BigInt(value);
    for (let index = width - 1; index >= 0; index -= 1) {
      out[index] = Number(rest & 0xffn);
      rest >>= 8n;
    }
    return out;
  }
  function field2(bytes) {
    return concat(integer(bytes.length, 4), bytes);
  }
  function random(length) {
    return crypto.getRandomValues(new Uint8Array(length));
  }
  function sameBytes(left, right) {
    if (left.length !== right.length) {
      return false;
    }
    let difference = 0;
    for (let index = 0; index < left.length; index += 1) {
      difference |= (left[index] ?? 0) ^ (right[index] ?? 0);
    }
    return difference === 0;
  }
  function wipe(...buffers) {
    for (const buffer of buffers) {
      buffer.fill(0);
    }
  }
  var Reader = class {
    #bytes;
    #offset = 0;
    constructor(bytes) {
      this.#bytes = bytes;
    }
    take(length) {
      if (length > this.#bytes.length - this.#offset) {
        throw new VaultCryptoError("malformed");
      }
      const out = this.#bytes.slice(this.#offset, this.#offset + length);
      this.#offset += length;
      return out;
    }
    integer(width) {
      let value = 0n;
      for (const byte of this.take(width)) {
        value = value << 8n | BigInt(byte);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new VaultCryptoError("malformed");
      }
      return Number(value);
    }
    field() {
      return this.take(this.integer(4));
    }
    rest() {
      return this.take(this.#bytes.length - this.#offset);
    }
    end() {
      if (this.#offset !== this.#bytes.length) {
        throw new VaultCryptoError("malformed");
      }
    }
  };
  function hex(bytes) {
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  }

  // src/chain-model.ts
  var OWNER_KEY_BYTES = 32;
  var isRecord3 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isText = (v) => typeof v === "string" && v !== "";
  function readChain(v) {
    if (!isRecord3(v) || !Array.isArray(v["statements"])) {
      return null;
    }
    const pinned = v["owner_signing"];
    const ownerSigning = pinned === null ? null : isText(pinned) ? decode(pinned) : void 0;
    if (ownerSigning === void 0 || ownerSigning !== null && ownerSigning.length !== OWNER_KEY_BYTES) {
      return null;
    }
    const statements = [];
    for (const [index, item] of v["statements"].entries()) {
      if (!isRecord3(item) || item["seq"] !== index + 1 || !isText(item["bytes"]) || !isText(item["signature"])) {
        return null;
      }
      const [bytes, signature] = [decode(item["bytes"]), decode(item["signature"])];
      if (bytes === null || signature === null) {
        return null;
      }
      statements.push({ bytes, signature });
    }
    return ownerSigning === null === (statements.length === 0) ? { ownerSigning, statements } : null;
  }
  function ownerPinFor(chain, role) {
    if (chain.ownerSigning !== null) {
      return chain.ownerSigning;
    }
    return role === "owner" ? "self" : "not-yet";
  }
  var needsGenesis = (chain, keys) => chain.ownerSigning === null && sameBytes(keys.ownerSigningPublic, keys.signingPublic);
  var genesisBody = (statement, ownerSigning) => ({
    seq: 1,
    bytes: encode(statement.bytes),
    signature: encode(statement.signature),
    owner_signing: encode(ownerSigning)
  });

  // src/crypto/keys.ts
  var PKCS8_PREFIX_X25519 = Uint8Array.of(48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 110, 4, 34, 4, 32);
  var PKCS8_PREFIX_ED25519 = Uint8Array.of(48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 112, 4, 34, 4, 32);
  var KEY_BYTES = 32;
  function exactKey(raw) {
    if (raw.length !== KEY_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    return raw;
  }
  async function ed25519PublicFromBytes(raw) {
    return crypto.subtle.importKey("raw", exactKey(raw), { name: "Ed25519" }, true, ["verify"]);
  }
  async function publicBytes(publicKey) {
    return new Uint8Array(await crypto.subtle.exportKey("raw", publicKey));
  }
  async function generatePair(algorithm, extractable = true) {
    const usages = algorithm === "X25519" ? ["deriveBits"] : ["sign", "verify"];
    const generated = await crypto.subtle.generateKey({ name: algorithm }, extractable, usages);
    if (!("privateKey" in generated)) {
      throw new VaultCryptoError("malformed");
    }
    return generated;
  }

  // src/crypto/labels.ts
  var PREFIX = "tessaridb-vault/v1/";
  var LABEL = {
    /** HKDF info for the unlock wrap key derived from the master key. */
    unlockWrap: `${PREFIX}unlock-wrap`,
    /** HKDF info for a revision's content key derived from its record key. */
    recordContent: `${PREFIX}record-content`,
    /** Associated-data prefix of a record body. */
    record: `${PREFIX}record`,
    /** HKDF info for a file's key derived from its record key. */
    file: `${PREFIX}file`,
    /** Associated-data prefix of every chunk of a file. */
    fileChunk: `${PREFIX}file-chunk`,
    /** Associated-data prefix of a membership private key wrapped under the unlock key. */
    memberKey: `${PREFIX}member-key`,
    /** Associated-data prefix of the company owner's public key, pinned under the unlock key in a member's key set. */
    ownerPin: `${PREFIX}owner-pin`,
    /** HPKE `info` prefix of a key wrap. */
    wrap: `${PREFIX}wrap`,
    /** Ed25519 domain prefix of a vouching statement. */
    statement: `${PREFIX}statement`,
    /** Ed25519 domain prefix of a revision signature. */
    revision: `${PREFIX}revision`
  };
  var FORMAT_VERSION = 1;
  var ALGORITHM = {
    recordBody: 1,
    keyWrap: 2,
    memberKey: 3,
    signature: 4,
    file: 5
  };

  // src/crypto/unlock.ts
  var MAX_WRAPS = 2 ** 20;
  var SALT_BYTES = 16;
  var MASTER_KEY_BYTES = 32;
  var KEY_ID_BYTES = 16;
  var NONCE_BYTES = 12;
  var DERIVED = 0;
  function exportsOf(instance) {
    const { memory, vault_alloc: alloc, vault_free: free, vault_derive: derive } = instance.exports;
    if (!(memory instanceof WebAssembly.Memory) || typeof alloc !== "function" || typeof free !== "function" || typeof derive !== "function") {
      throw new VaultCryptoError("kdf-refused");
    }
    return {
      memory,
      alloc: (length) => Number(alloc(length)),
      free: (pointer, length) => {
        free(pointer, length);
      },
      derive: (...args) => Number(derive(...args))
    };
  }
  async function deriveMasterKey(module2, password, salt, params) {
    if (salt.length !== SALT_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    const wasm = exportsOf(await WebAssembly.instantiate(module2, {}));
    const passwordBytes = utf8(password);
    const passwordLength = Math.max(passwordBytes.length, 1);
    const passwordAt = wasm.alloc(passwordLength);
    const saltAt = wasm.alloc(SALT_BYTES);
    const outAt = wasm.alloc(MASTER_KEY_BYTES);
    try {
      const memory = new Uint8Array(wasm.memory.buffer);
      memory.set(passwordBytes, passwordAt);
      memory.set(salt, saltAt);
      const status = wasm.derive(
        passwordAt,
        passwordBytes.length,
        saltAt,
        params.memoryKib,
        params.passes,
        params.lanes,
        outAt
      );
      if (status !== DERIVED) {
        throw new VaultCryptoError("kdf-refused");
      }
      return new Uint8Array(wasm.memory.buffer).slice(outAt, outAt + MASTER_KEY_BYTES);
    } finally {
      wipe(passwordBytes);
      wasm.free(passwordAt, passwordLength);
      wasm.free(saltAt, SALT_BYTES);
      wasm.free(outAt, MASTER_KEY_BYTES);
    }
  }
  function associatedData(context2) {
    if (context2.keyId.length !== KEY_ID_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    return concat(
      utf8(LABEL.memberKey),
      field2(utf8(context2.account)),
      field2(utf8(context2.company)),
      field2(utf8(context2.kind)),
      field2(context2.keyId)
    );
  }
  function pinData(pin) {
    if (pin.keyId.length !== KEY_ID_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    return concat(utf8(LABEL.ownerPin), field2(utf8(pin.account)), field2(utf8(pin.company)), field2(pin.keyId));
  }
  var UnlockKey = class _UnlockKey {
    #key;
    #wrapsMade;
    constructor(key, wrapsMade) {
      this.#key = key;
      this.#wrapsMade = wrapsMade;
    }
    /** UK = HKDF(MK, "tessaridb-vault/v1/unlock-wrap"); MK is wiped. `wrapsMade` is the stored count. */
    static async fromMasterKey(masterKey, wrapsMade) {
      if (masterKey.length !== MASTER_KEY_BYTES || !Number.isSafeInteger(wrapsMade) || wrapsMade < 0) {
        wipe(masterKey);
        throw new VaultCryptoError("malformed");
      }
      if (wrapsMade > MAX_WRAPS) {
        wipe(masterKey);
        throw new VaultCryptoError("wrap-bound");
      }
      const material = await crypto.subtle.importKey("raw", masterKey, "HKDF", false, ["deriveKey"]);
      wipe(masterKey);
      const key = await crypto.subtle.deriveKey(
        { name: "HKDF", hash: "SHA-256", salt: new Uint8Array(0), info: utf8(LABEL.unlockWrap) },
        material,
        { name: "AES-GCM", length: 256 },
        false,
        ["wrapKey", "unwrapKey"]
      );
      return new _UnlockKey(key, wrapsMade);
    }
    get wrapsMade() {
      return this.#wrapsMade;
    }
    /** Wrap a freshly generated, still extractable private key; refused once 2^20 wraps were made. */
    async wrap(privateKey, context2) {
      if (this.#wrapsMade >= MAX_WRAPS) {
        throw new VaultCryptoError("wrap-bound");
      }
      this.#wrapsMade += 1;
      const nonce = random(NONCE_BYTES);
      const sealed = await crypto.subtle.wrapKey("pkcs8", privateKey, this.#key, {
        name: "AES-GCM",
        iv: nonce,
        additionalData: associatedData(context2)
      });
      return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.memberKey), context2.keyId, nonce, new Uint8Array(sealed));
    }
    /** Seal the company owner's Ed25519 public key under UK, to be pinned in this member's key set; one more wrap. */
    async sealPin(ownerPublic, pin) {
      if (this.#wrapsMade >= MAX_WRAPS) {
        throw new VaultCryptoError("wrap-bound");
      }
      this.#wrapsMade += 1;
      const nonce = random(NONCE_BYTES);
      const sealed = await crypto.subtle.wrapKey("raw", ownerPublic, this.#key, {
        name: "AES-GCM",
        iv: nonce,
        additionalData: pinData(pin)
      });
      return { nonce, sealed: new Uint8Array(sealed) };
    }
    /** Open the pinned owner key; a pin sealed for another member, company or key set is `unauthentic`. */
    async openPin(pin, nonce, sealed) {
      if (nonce.length !== NONCE_BYTES) {
        throw new VaultCryptoError("malformed");
      }
      try {
        return await crypto.subtle.unwrapKey(
          "raw",
          sealed,
          this.#key,
          { name: "AES-GCM", iv: nonce, additionalData: pinData(pin) },
          { name: "Ed25519" },
          true,
          ["verify"]
        );
      } catch {
        throw new VaultCryptoError("unauthentic");
      }
    }
    /** Unwrap into a non-extractable key: X25519 for agreement, Ed25519 for signing. */
    async unwrap(context2, wrapped) {
      const reader = new Reader(wrapped);
      if (reader.integer(1) !== FORMAT_VERSION) {
        throw new VaultCryptoError("unknown-version");
      }
      if (reader.integer(1) !== ALGORITHM.memberKey) {
        throw new VaultCryptoError("unknown-algorithm");
      }
      const keyId = reader.take(KEY_ID_BYTES);
      const nonce = reader.take(NONCE_BYTES);
      const sealed = reader.rest();
      if (!sameBytes(keyId, context2.keyId)) {
        throw new VaultCryptoError("key-mismatch");
      }
      const agreement = context2.kind === "agreement";
      try {
        return await crypto.subtle.unwrapKey(
          "pkcs8",
          sealed,
          this.#key,
          { name: "AES-GCM", iv: nonce, additionalData: associatedData(context2) },
          { name: agreement ? "X25519" : "Ed25519" },
          false,
          agreement ? ["deriveBits"] : ["sign"]
        );
      } catch {
        throw new VaultCryptoError("unauthentic");
      }
    }
  };

  // src/crypto/key-set.ts
  var NEW_KDF = { memoryKib: 65536, passes: 3, lanes: 4 };
  var SALT_BYTES2 = 16;
  var KEY_ID_BYTES2 = 16;
  var PIN_NONCE_BYTES = 12;
  var KEY_SET_VERSION = 2;
  var context = (account, company, kind, keyId) => ({
    account,
    company,
    kind,
    keyId
  });
  async function unlockKey(module2, password, salt, kdf, wrapsMade) {
    return UnlockKey.fromMasterKey(await deriveMasterKey(module2, password, salt, kdf), wrapsMade);
  }
  function parse(stored) {
    const reader = new Reader(stored.wrapped);
    if (reader.integer(1) !== KEY_SET_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    const wrapsMade = reader.integer(4);
    const agreementWrap = reader.field();
    const signingWrap = reader.field();
    const pinNonce = reader.take(PIN_NONCE_BYTES);
    const pin = reader.field();
    reader.end();
    return { wrapsMade, agreementWrap, signingWrap, pinNonce, pin };
  }
  async function openWith(key, account, company, stored) {
    const { agreementWrap, signingWrap, pinNonce, pin } = parse(stored);
    const owner = await key.openPin({ account, company, keyId: stored.keyId }, pinNonce, pin);
    return {
      keyId: stored.keyId,
      agreementPrivate: await key.unwrap(context(account, company, "agreement", stored.keyId), agreementWrap),
      signingPrivate: await key.unwrap(context(account, company, "signing", stored.keyId), signingWrap),
      agreementPublic: stored.agreementPublic,
      signingPublic: stored.signingPublic,
      ownerSigningPublic: await publicBytes(owner)
    };
  }
  async function openKeySet(module2, password, account, company, stored) {
    const { wrapsMade } = parse(stored);
    return openWith(await unlockKey(module2, password, stored.salt, stored.kdf, wrapsMade), account, company, stored);
  }
  async function createKeySet(module2, password, account, company, ownerSigningPublic) {
    const salt = random(SALT_BYTES2);
    const keyId = random(KEY_ID_BYTES2);
    const key = await unlockKey(module2, password, salt, NEW_KDF, 0);
    const agreement = await generatePair("X25519");
    const signing = await generatePair("Ed25519");
    const agreementWrap = await key.wrap(agreement.privateKey, context(account, company, "agreement", keyId));
    const signingWrap = await key.wrap(signing.privateKey, context(account, company, "signing", keyId));
    const signingPublic = await publicBytes(signing.publicKey);
    const owner = await ed25519PublicFromBytes(ownerSigningPublic === "self" ? signingPublic : ownerSigningPublic);
    const pin = await key.sealPin(owner, { account, company, keyId });
    const stored = {
      keyId,
      agreementPublic: await publicBytes(agreement.publicKey),
      signingPublic,
      wrapped: concat(
        Uint8Array.of(KEY_SET_VERSION),
        integer(key.wrapsMade, 4),
        field2(agreementWrap),
        field2(signingWrap),
        pin.nonce,
        field2(pin.sealed)
      ),
      salt,
      kdf: NEW_KDF
    };
    return { stored, open: await openWith(key, account, company, stored) };
  }

  // src/crypto/signatures.ts
  async function sign(privateKey, message) {
    const signature = new Uint8Array(await crypto.subtle.sign("Ed25519", privateKey, message));
    return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.signature), signature);
  }

  // src/crypto/statements.ts
  var KEY_BYTES2 = 32;
  var DIGEST_BYTES = 32;
  var KINDS = ["member", "admin", "group", "revoke", "recovery"];
  function encode2(company, seq, prev, fields, signerPublic) {
    return concat(
      utf8(LABEL.statement),
      Uint8Array.of(FORMAT_VERSION, KINDS.indexOf(fields.kind) + 1),
      field2(utf8(company)),
      integer(seq, 8),
      prev,
      field2(utf8(fields.subject)),
      field2(fields.agreementPublic),
      field2(fields.signingPublic),
      signerPublic
    );
  }
  function decode2(bytes) {
    const reader = new Reader(bytes);
    if (!sameBytes(reader.take(utf8(LABEL.statement).length), utf8(LABEL.statement))) {
      throw new VaultCryptoError("malformed");
    }
    if (reader.integer(1) !== FORMAT_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    const kind = KINDS[reader.integer(1) - 1];
    if (kind === void 0) {
      throw new VaultCryptoError("chain-broken");
    }
    const decoder = new TextDecoder("utf-8", { fatal: true });
    const text2 = (raw) => {
      try {
        return decoder.decode(raw);
      } catch {
        throw new VaultCryptoError("malformed");
      }
    };
    const decoded = {
      kind,
      company: text2(reader.field()),
      seq: reader.integer(8),
      prev: reader.take(DIGEST_BYTES),
      subject: text2(reader.field()),
      agreementPublic: reader.field(),
      signingPublic: reader.field(),
      signerPublic: reader.take(KEY_BYTES2)
    };
    reader.end();
    return decoded;
  }
  async function digest(statement) {
    return new Uint8Array(await crypto.subtle.digest("SHA-256", concat(statement.bytes, statement.signature)));
  }
  async function appendStatement(company, previous, fields, signer) {
    const seq = previous === void 0 ? 1 : decode2(previous.bytes).seq + 1;
    const prev = previous === void 0 ? new Uint8Array(DIGEST_BYTES) : await digest(previous);
    const bytes = encode2(company, seq, prev, fields, signer.publicBytes);
    return { bytes, signature: await sign(signer.privateKey, bytes) };
  }

  // src/unlock-model.ts
  var isRecord4 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isText2 = (v) => typeof v === "string" && v !== "";
  var isCount = (v) => typeof v === "number" && Number.isSafeInteger(v) && v > 0;
  function readMe(v) {
    if (!isRecord4(v) || !isRecord4(v["account"]) || typeof v["platform_admin"] !== "boolean") {
      return null;
    }
    const { id, name, email } = v["account"];
    return isText2(id) && isText2(name) && isText2(email) ? { account: id, name, email, platformAdmin: v["platform_admin"] } : null;
  }
  var isRole = (v) => v === "owner" || v === "admin" || v === "member";
  function readCompanies(v) {
    if (!isRecord4(v) || !Array.isArray(v["companies"])) {
      return null;
    }
    const companies = [];
    for (const item of v["companies"]) {
      if (!isRecord4(item) || !isText2(item["id"]) || !isText2(item["name"]) || !isText2(item["state"])) {
        return null;
      }
      const role = item["role"];
      if (role !== null && !isRole(role)) {
        return null;
      }
      companies.push({ id: item["id"], name: item["name"], active: item["state"] === "active", role });
    }
    return companies;
  }
  var unlockable = (companies) => companies.filter((company) => company.active && company.role !== null);
  function keyIdBytes(text2) {
    if (!/^[0-9a-f]{32}$/.test(text2)) {
      return null;
    }
    return Uint8Array.from(text2.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
  }
  function readOwnKeys(v) {
    if (!isRecord4(v) || !isRecord4(v["kdf"]) || !isText2(v["key_id"])) {
      return null;
    }
    const { salt, memory_kib: memoryKib, passes, lanes } = v["kdf"];
    if (!isText2(salt) || !isCount(memoryKib) || !isCount(passes) || !isCount(lanes)) {
      return null;
    }
    const keyId = keyIdBytes(v["key_id"]);
    const parts = [v["agreement_public"], v["signing_public"], v["wrapped"], salt].map((part) => isText2(part) ? decode(part) : null);
    const [agreementPublic, signingPublic, wrapped, saltBytes] = parts;
    if (keyId === null || agreementPublic == null || signingPublic == null || wrapped == null || saltBytes == null) {
      return null;
    }
    return { keyId, agreementPublic, signingPublic, wrapped, salt: saltBytes, kdf: { memoryKib, passes, lanes } };
  }
  var keySetBody = (stored) => ({
    key_id: hex(stored.keyId),
    agreement_public: encode(stored.agreementPublic),
    signing_public: encode(stored.signingPublic),
    wrapped: encode(stored.wrapped),
    kdf: { salt: encode(stored.salt), memory_kib: stored.kdf.memoryKib, passes: stored.kdf.passes, lanes: stored.kdf.lanes }
  });
  var UNLOCK_MINIMUM = 12;
  function newPasswordProblem(password, repeated, signInEmail) {
    if ([...password].length < UNLOCK_MINIMUM) {
      return `Use at least ${UNLOCK_MINIMUM} characters. A phrase of a few words is easiest to remember.`;
    }
    if (password !== repeated) {
      return "The two passwords don't match. Type the same one twice.";
    }
    if (password.toLowerCase() === signInEmail.toLowerCase()) {
      return "Don't use your email address as your unlock password.";
    }
    return null;
  }
  var readKept = (sent) => (v) => isRecord4(v) && v["key_id"] === hex(sent) ? hex(sent) : null;

  // src/vault.ts
  var IDLE_MINUTES = 15;
  var held = null;
  var listeners = /* @__PURE__ */ new Set();
  var arm = null;
  var unlocked = () => held;
  function hold(opened) {
    held = opened;
    arm?.();
  }
  function lock() {
    if (held === null) {
      return;
    }
    held = null;
    for (const listener of listeners) {
      listener();
    }
  }
  function onLock(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }
  function lockWhenIdle(target, minutes, clock, events = ["pointerdown", "keydown", "wheel", "touchstart"]) {
    let timer = null;
    const restart = () => {
      if (timer !== null) {
        clock.clear(timer);
      }
      timer = held === null ? null : clock.set(lock, minutes * 6e4);
    };
    for (const name of events) {
      target.addEventListener(name, restart, { passive: true });
    }
    arm = restart;
    restart();
    return () => {
      arm = null;
      if (timer !== null) {
        clock.clear(timer);
      }
      for (const name of events) {
        target.removeEventListener(name, restart);
      }
    };
  }

  // src/unlock.ts
  var module = null;
  function cryptoModule() {
    module ??= WebAssembly.compileStreaming(fetch("/vault-crypto.wasm"));
    return module;
  }
  var signOutHere = () => {
  };
  function card2(title, lead, ...body) {
    const leave = el("button", { type: "button", class: "quiet" }, "Sign out");
    leave.addEventListener("click", () => signOutHere());
    return el(
      "div",
      { class: "gate" },
      el(
        "form",
        { class: "card", novalidate: "" },
        el("img", { class: "logo", src: "/favicon.svg", alt: "", width: "48", height: "48" }),
        heading(title),
        el("p", { class: "muted" }, lead),
        ...body,
        leave
      )
    );
  }
  function unlockFailure(problem) {
    if (problem instanceof VaultCryptoError && problem.reason === "unauthentic") {
      return "That unlock password doesn't open your keys. Check it and try again; it is not your sign-in password.";
    }
    if (problem instanceof VaultCryptoError && problem.reason === "kdf-refused") {
      return "Your keys were made with settings this page can't run. Tell your admin.";
    }
    return "This page couldn't open your keys. Reload the page and try again.";
  }
  var loadFailure = (failure) => failure.code === "network" ? "The vault didn't answer. Check your connection, then try again." : "The vault couldn't load your keys. Try again in a minute.";
  var chainPath = (company) => `/api/companies/${encodeURIComponent(company.id)}/statements`;
  async function beginChain(company, me, keys) {
    const fields = { kind: "member", subject: me.account, agreementPublic: keys.agreementPublic, signingPublic: keys.signingPublic };
    const statement = await appendStatement(company.id, void 0, fields, { privateKey: keys.signingPrivate, publicBytes: keys.signingPublic });
    const answer = await call("POST", chainPath(company), ignored, { body: genesisBody(statement, keys.signingPublic) });
    return answer.ok;
  }
  async function chainOwed(company, me, keys) {
    if (company.role !== "owner") {
      return true;
    }
    const chain = await call("GET", chainPath(company), readChain);
    return chain.ok && (!needsGenesis(chain.value, keys) || await beginChain(company, me, keys));
  }
  var CHAIN_FAILURE = "The vault couldn't record you as your company's owner. Try again in a minute.";
  function companyChoice(companies) {
    const first = companies[0];
    if (first === void 0) {
      throw new RangeError("no company to choose");
    }
    if (companies.length === 1) {
      return { row: null, chosen: () => first };
    }
    const inputs = companies.map((company, at) => {
      const input = el("input", { type: "radio", name: "company", id: `company-${at}`, value: company.id });
      input.checked = at === 0;
      return input;
    });
    const row = el(
      "fieldset",
      { class: "field choices" },
      el("legend", {}, "Company"),
      ...inputs.flatMap((input, at) => [el("div", { class: "choice" }, input, el("label", { for: input.id }, companies[at]?.name ?? ""))])
    );
    return { row, chosen: () => companies.find((company) => company.id === inputs.find((input) => input.checked)?.value) ?? first };
  }
  function unlockForm(main, me, company, stored, choice, done) {
    const password = field("unlock-password", "Unlock password", { type: "password", autocomplete: "current-password", required: "" });
    const submit = el("button", { type: "submit", class: "primary" }, "Unlock");
    const said = el("div", { "aria-live": "polite" });
    const view2 = card2("Unlock your vault", `Your records in ${company.name} open here, in this browser.`, choice, password.row, submit, said);
    view2.querySelector("form")?.addEventListener("submit", async (event) => {
      event.preventDefault();
      submit.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Opening your keys. This takes a second."));
      try {
        const keys = await openKeySet(await cryptoModule(), password.input.value, me.account, company.id, stored);
        password.input.value = "";
        if (!await chainOwed(company, me, keys)) {
          fill(said, refusal(CHAIN_FAILURE));
          return;
        }
        hold({ company: company.id, account: me.account, keys });
        done();
      } catch (problem) {
        password.input.value = "";
        password.input.setAttribute("aria-invalid", "true");
        fill(said, refusal(unlockFailure(problem)));
        password.input.focus();
      } finally {
        submit.disabled = false;
      }
    });
    fill(main, view2);
    password.input.focus();
  }
  function firstUnlockForm(main, me, company, owner, choice, done) {
    const password = field(
      "new-unlock-password",
      "New unlock password",
      { type: "password", autocomplete: "new-password", required: "" },
      "At least 12 characters. Not your sign-in password: the vault can't reset this one."
    );
    const repeated = field("repeat-unlock-password", "Type it again", { type: "password", autocomplete: "new-password", required: "" });
    const submit = el("button", { type: "submit", class: "primary" }, "Set unlock password");
    const said = el("div", { "aria-live": "polite" });
    const view2 = card2(
      "Set your unlock password",
      `Your records in ${company.name} are encrypted with keys this browser makes now. The unlock password opens them.`,
      choice,
      password.row,
      repeated.row,
      submit,
      said
    );
    view2.querySelector("form")?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const problem = newPasswordProblem(password.input.value, repeated.input.value, me.email);
      if (problem !== null) {
        fill(said, refusal(problem));
        password.input.focus();
        return;
      }
      submit.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Making your keys. This takes a few seconds."));
      try {
        const made = await createKeySet(await cryptoModule(), password.input.value, me.account, company.id, owner);
        const answer = await call("PUT", `/api/companies/${encodeURIComponent(company.id)}/keys/me`, readKept(made.stored.keyId), {
          body: keySetBody(made.stored)
        });
        if (!answer.ok) {
          fill(said, refusal(loadFailure(answer)));
          return;
        }
        password.input.value = "";
        repeated.input.value = "";
        if (owner === "self" && !await beginChain(company, me, made.open)) {
          fill(said, refusal(CHAIN_FAILURE));
          return;
        }
        hold({ company: company.id, account: me.account, keys: made.open });
        done();
      } catch (problemMaking) {
        fill(said, refusal(unlockFailure(problemMaking)));
      } finally {
        submit.disabled = false;
      }
    });
    fill(main, view2);
    password.input.focus();
  }
  async function unlock(main, done, signOut2) {
    signOutHere = signOut2;
    fill(main, card2("Unlock your vault", "Loading your companies."));
    const [me, companies] = await Promise.all([call("GET", "/api/me", readMe), call("GET", "/api/companies", readCompanies)]);
    if (!me.ok) {
      fill(main, card2("Unlock your vault", "", refusal(loadFailure(me))));
      return;
    }
    if (!companies.ok) {
      fill(main, card2("Unlock your vault", "", refusal(loadFailure(companies))));
      return;
    }
    const open2 = unlockable(companies.value);
    if (open2.length === 0) {
      fill(
        main,
        card2(
          "No company yet",
          me.value.platformAdmin ? "You're a platform admin and not a member of any company, so there are no records to unlock." : "You aren't a member of an active company yet. Ask your company's admin to add you."
        )
      );
      return;
    }
    const choice = companyChoice(open2);
    const firstTime = async (company) => {
      const chain = await call("GET", chainPath(company), readChain);
      if (!chain.ok) {
        fill(main, card2("Unlock your vault", "", choice.row, refusal(loadFailure(chain))));
        return;
      }
      const owner = ownerPinFor(chain.value, company.role);
      if (owner === "not-yet") {
        const lead = "Your company's owner hasn't opened the vault yet. Once they have, you can set your unlock password here.";
        fill(main, card2("Waiting for your company's owner", lead, choice.row));
        return;
      }
      firstUnlockForm(main, me.value, company, owner, choice.row, done);
    };
    const draw2 = async () => {
      const company = choice.chosen();
      const keys = await call("GET", `/api/companies/${encodeURIComponent(company.id)}/keys/me`, readOwnKeys);
      if (keys.ok) {
        unlockForm(main, me.value, company, keys.value, choice.row, done);
      } else if (keys.status === 404) {
        await firstTime(company);
      } else {
        fill(main, card2("Unlock your vault", "", choice.row, refusal(loadFailure(keys))));
      }
    };
    choice.row?.addEventListener("change", () => void draw2());
    await draw2();
  }

  // src/app.ts
  var history = browserHistory(window);
  var notice = null;
  var signedIn = () => bearer() !== null;
  var isUnlocked = () => unlocked() !== null;
  var session = needsSession(signedIn, () => history.current());
  var open = every(session, needsUnlock(isUnlocked, () => history.current()));
  var routes = defineRoutes({
    records: { path: "/", guard: open },
    signIn: { path: "/sign-in", query: { next: text } },
    unlock: { path: "/unlock", query: { next: text }, guard: session }
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
    document.body.classList.toggle("signed-out", !signedIn() || !isUnlocked());
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
      case "unlock": {
        const next = outcome.match.query.next;
        if (isUnlocked()) {
          go(afterUnlock(next));
          return;
        }
        void unlock(view2, () => go(afterUnlock(next)), () => void signOut());
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
  async function signOut() {
    lock();
    await call("DELETE", "/api/session", ignored);
    forget();
    go("#/sign-in");
  }
  function signOutButton() {
    document.getElementById("sign-out")?.addEventListener("click", () => void signOut());
  }
  function locking() {
    document.getElementById("lock")?.addEventListener("click", lock);
    lockWhenIdle(window, IDLE_MINUTES, { set: (run, ms) => window.setTimeout(run, ms), clear: (handle) => window.clearTimeout(Number(handle)) });
    onLock(() => {
      if (signedIn()) {
        go(`#/unlock?next=${encodeURIComponent(history.current().replace(/^#/, ""))}`);
      }
    });
  }
  var view = document.getElementById("view");
  if (view !== null) {
    themed();
    signOutButton();
    locking();
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
