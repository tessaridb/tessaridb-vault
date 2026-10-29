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
      made["vault-reason"] = encodeURIComponent(options.reason);
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
    const text3 = response.status === 204 ? "" : await response.text().catch(() => void 0);
    if (text3 === void 0) {
      parsed = void 0;
    } else if (text3 !== "") {
      try {
        parsed = JSON.parse(text3);
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
  async function callBytes(method, route, body) {
    const sent = headers({});
    sent["accept"] = "application/octet-stream, application/json";
    if (body !== void 0) {
      sent["content-type"] = "application/octet-stream";
    }
    let response;
    try {
      response = await fetch(route, { method, credentials: "same-origin", headers: sent, ...body === void 0 ? {} : { body } });
    } catch {
      return { ok: false, status: 0, code: "network", message: "The vault did not answer." };
    }
    if (response.ok) {
      return { ok: true, value: new Uint8Array(await response.arrayBuffer()) };
    }
    let refusal2 = null;
    try {
      refusal2 = readRefusal(JSON.parse(await response.text()));
    } catch {
      refusal2 = null;
    }
    return {
      ok: false,
      status: response.status,
      code: refusal2?.code ?? "unknown",
      message: refusal2?.message ?? `The vault refused the request (status ${response.status}).`
    };
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
  function at(id) {
    const found = document.getElementById(id);
    if (found === null) {
      throw new Error(`the page has no element #${id}`);
    }
    return found;
  }
  function fill(parent, ...children) {
    parent.replaceChildren(...children.filter((child) => child !== null && child !== false));
  }
  function announce(words) {
    at("status-line").textContent = words;
  }
  function field(id, label, attributes = {}, hint) {
    const input = el("input", { id, name: id, type: "text", ...attributes });
    const described = hint === void 0 ? null : el("p", { id: `${id}-hint`, class: "hint" }, hint);
    if (described !== null) {
      input.setAttribute("aria-describedby", described.id);
    }
    return { row: el("div", { class: "field" }, el("label", { for: id }, label), input, described), input };
  }

  // src/guards.ts
  var SIGN_IN = "/sign-in";
  var UNLOCK = "/unlock";
  function gated(gate, open3, current) {
    return () => open3() ? { kind: "allow" } : { kind: "redirect", to: `#${gate}?next=${encodeURIComponent(current().replace(/^#/, ""))}` };
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
    const at2 = bare.indexOf("?");
    const path = at2 === -1 ? bare : bare.slice(0, at2);
    return { path: path === "" ? "/" : path, query: at2 === -1 ? "" : bare.slice(at2 + 1) };
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

  // src/crypto/errors.ts
  var VaultCryptoError = class extends Error {
    reason;
    constructor(reason) {
      super(`refused: ${reason}`);
      this.name = "VaultCryptoError";
      this.reason = reason;
    }
  };

  // src/base64url.ts
  function encode(bytes) {
    return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join("")).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
  }
  function decode(text3) {
    if (!/^[A-Za-z0-9_-]*$/.test(text3) || text3.length % 4 === 1) {
      return null;
    }
    const standard = text3.replaceAll("-", "+").replaceAll("_", "/");
    return Uint8Array.from(atob(standard.padEnd(Math.ceil(standard.length / 4) * 4, "=")), (character) => character.charCodeAt(0));
  }

  // src/crypto/bytes.ts
  var ENCODER = new TextEncoder();
  function utf8(text3) {
    return ENCODER.encode(text3);
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

  // src/unlock-model.ts
  var isRecord2 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isText = (v) => typeof v === "string" && v !== "";
  var isCount = (v) => typeof v === "number" && Number.isSafeInteger(v) && v > 0;
  function readMe(v) {
    if (!isRecord2(v) || !isRecord2(v["account"]) || typeof v["platform_admin"] !== "boolean") {
      return null;
    }
    const { id, name, email } = v["account"];
    return isText(id) && isText(name) && isText(email) ? { account: id, name, email, platformAdmin: v["platform_admin"] } : null;
  }
  var isRole = (v) => v === "owner" || v === "admin" || v === "member";
  function readCompanies(v) {
    if (!isRecord2(v) || !Array.isArray(v["companies"])) {
      return null;
    }
    const companies = [];
    for (const item of v["companies"]) {
      if (!isRecord2(item) || !isText(item["id"]) || !isText(item["name"]) || !isText(item["state"])) {
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
  function keyIdBytes(text3) {
    if (!/^[0-9a-f]{32}$/.test(text3)) {
      return null;
    }
    return Uint8Array.from(text3.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
  }
  function readOwnKeys(v) {
    if (!isRecord2(v) || !isRecord2(v["kdf"]) || !isText(v["key_id"])) {
      return null;
    }
    const { salt, memory_kib: memoryKib, passes, lanes } = v["kdf"];
    if (!isText(salt) || !isCount(memoryKib) || !isCount(passes) || !isCount(lanes)) {
      return null;
    }
    const keyId = keyIdBytes(v["key_id"]);
    const parts = [v["agreement_public"], v["signing_public"], v["wrapped"], salt].map((part) => isText(part) ? decode(part) : null);
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
  var readKept = (sent) => (v) => isRecord2(v) && v["key_id"] === hex(sent) ? hex(sent) : null;
  function firstSave(refused, chained) {
    if (refused === "keys_exist") {
      return { kind: "kept", words: "You already set an unlock password for this company. Unlock with it." };
    }
    if (refused !== null) {
      return { kind: "failed" };
    }
    return chained ? { kind: "done" } : {
      kind: "kept",
      words: "Your keys are made and kept, but the vault couldn't record you as your company's owner yet. Unlock with the password you just set to try again."
    };
  }

  // src/records/share-targets.ts
  var isRecord3 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isId = (v) => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
  var isText2 = (v) => typeof v === "string" && v !== "";
  function readMemberList(v) {
    if (!isRecord3(v) || !Array.isArray(v["members"])) {
      return null;
    }
    const members = v["members"].map((member) => {
      if (!isRecord3(member) || !isRecord3(member["account"]) || typeof member["state"] !== "string") {
        return null;
      }
      const { id, name, email } = member["account"];
      const { role } = member;
      return isId(id) && isText2(name) && isText2(email) && (role === "owner" || role === "admin" || role === "member") ? { account: id, name, email, role, active: member["state"] === "active" } : null;
    });
    return members.every((member) => member !== null) ? members : null;
  }
  function readGroupList(v) {
    if (!isRecord3(v) || !Array.isArray(v["groups"])) {
      return null;
    }
    const groups = v["groups"].map((group) => {
      if (!isRecord3(group) || !isId(group["id"]) || !isText2(group["name"])) {
        return null;
      }
      const { kind } = group;
      return kind === "company" || kind === "department" || kind === "team" ? { id: group["id"], kind, name: group["name"] } : null;
    });
    return groups.every((group) => group !== null) ? groups : null;
  }
  function shareTargets(members, groups, directory, groupKeys, author, shared) {
    const open3 = (id) => id !== author && !shared.includes(id);
    const people = members.filter((member) => member.active && open3(member.account)).map((member) => {
      const listed = directory.get(member.account);
      const keyId = listed === void 0 ? null : keyIdBytes(listed.keyId);
      return listed === void 0 || keyId === null ? { id: member.account, label: member.name, detail: member.email, recipient: null, missing: "hasn't set an unlock password yet" } : { id: member.account, label: member.name, detail: member.email, recipient: { recipient: { kind: "member", account: member.account }, keyId, listed: listed.agreementPublic } };
    });
    const teams = groups.filter((group) => open3(group.id)).map((group) => {
      const listed = groupKeys.get(group.id);
      const keyId = listed === void 0 ? null : keyIdBytes(listed.keyId);
      const detail = groupDetail(group.kind);
      return listed === void 0 || keyId === null ? { id: group.id, label: group.name, detail, recipient: null, missing: "has no key yet" } : { id: group.id, label: group.name, detail, recipient: { recipient: { kind: "group", group: group.id }, keyId, listed: listed.public } };
    });
    return [...people, ...teams];
  }
  function filterTargets(targets, text3) {
    const wanted = text3.trim().toLowerCase();
    return targets.filter((target) => `${target.label}
${target.detail}`.toLowerCase().includes(wanted));
  }
  function sharedRows(shares, members, groups) {
    return shares.map((share2) => {
      const { target, right } = share2;
      if (share2.kind === "member") {
        const member = members.find((each) => each.account === target);
        return member === void 0 ? { target, label: "A member no longer listed", detail: "", right } : { target, label: member.name, detail: member.email, right };
      }
      const group = groups.find((each) => each.id === target);
      return group === void 0 ? { target, label: "A group no longer listed", detail: "", right } : { target, label: group.name, detail: groupDetail(group.kind), right };
    });
  }
  var groupDetail = (kind) => kind === "company" ? "Everyone in the company" : kind === "department" ? "Department" : "Team";

  // src/records-model.ts
  var isRecord4 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isText3 = (v) => typeof v === "string" && v !== "";
  var isId2 = (v) => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
  var isWhole = (v) => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
  function readRow(v) {
    if (!isRecord4(v)) {
      return null;
    }
    const { id, record_type: recordType, revision, key_id: keyId, author, created, updated, rotation_due: rotationDue } = v;
    return isId2(id) && isText3(recordType) && isWhole(revision) && revision > 0 && isId2(keyId) && isId2(author) && isWhole(created) && isWhole(updated) && typeof rotationDue === "boolean" ? { id, recordType, revision, keyId, author, created, updated, rotationDue } : null;
  }
  function readRecipient(v) {
    if (typeof v !== "string") {
      return null;
    }
    const [kind, keyId, extra] = v.split(":");
    return (kind === "member" || kind === "group") && isId2(keyId) && extra === void 0 ? { kind, keyId } : null;
  }
  function readOpened(v) {
    if (!isRecord4(v)) {
      return null;
    }
    const row2 = readRow(v["record"]);
    const recipient = readRecipient(v["recipient"]);
    const [body, signature, wrap] = [v["body"], v["signature"], v["wrap"]].map((part) => isText3(part) ? decode(part) : null);
    const authorKey = v["author_key"];
    const through = v["through"];
    const revisedBy = v["revised_by"];
    return row2 !== null && recipient !== null && body != null && signature != null && wrap != null && isId2(authorKey) && isId2(revisedBy) && Array.isArray(through) && through.every(isId2) ? { row: row2, body, signature, authorKey, revisedBy, recipient, wrap, through } : null;
  }
  function readOpenedPage(v) {
    if (!isRecord4(v) || !Array.isArray(v["records"])) {
      return null;
    }
    const next = v["next"];
    if (next !== null && !isId2(next)) {
      return null;
    }
    const records = [];
    for (const item of v["records"]) {
      const opened = readOpened(item);
      if (opened === null) {
        return null;
      }
      records.push(opened);
    }
    return { records, next };
  }
  var openedPagePath = (company, cursor) => `/api/companies/${encodeURIComponent(company)}/records?opened=true${cursor === null ? "" : `&after=${encodeURIComponent(cursor)}`}`;
  function readGroupIds(v) {
    if (!isRecord4(v) || !Array.isArray(v["groups"])) {
      return null;
    }
    const ids = v["groups"].map((group) => isRecord4(group) && isId2(group["id"]) ? group["id"] : null);
    return ids.every((id) => id !== null) ? ids : null;
  }
  function readGroupKey(v) {
    if (!isRecord4(v) || typeof v["key_id"] !== "string" || !isText3(v["public"])) {
      return null;
    }
    const keyId = keyIdBytes(v["key_id"]);
    const listed = decode(v["public"]);
    const wrapped = v["wrap"];
    const wrap = wrapped === null ? null : isText3(wrapped) ? decode(wrapped) ?? void 0 : void 0;
    return keyId !== null && listed !== null && listed.length === 32 && wrap !== void 0 ? { keyId, public: listed, wrap } : null;
  }

  // src/records/audience.ts
  var ORDER = { company: 0, department: 1, team: 2 };
  function companyAndGroups(company, groups) {
    const itself = { id: company.id, kind: "company", name: company.name };
    return [itself, ...groups.filter((group) => group.id !== company.id)].sort(
      (one, other) => ORDER[one.kind] - ORDER[other.kind] || one.name.localeCompare(other.name)
    );
  }
  function audienceChoices(company, groups, groupKeys) {
    return shareTargets([], companyAndGroups(company, groups), /* @__PURE__ */ new Map(), groupKeys, "", []);
  }
  function groupsToOpen(company, ids) {
    return [company, ...ids.filter((id) => id !== company)];
  }

  // src/icons.ts
  var SVG = "http://www.w3.org/2000/svg";
  var box = (x, y, w, h, r) => `M${x + r} ${y}h${w - 2 * r}a${r} ${r} 0 0 1 ${r} ${r}v${h - 2 * r}a${r} ${r} 0 0 1 -${r} ${r}h-${w - 2 * r}a${r} ${r} 0 0 1 -${r} -${r}v-${h - 2 * r}a${r} ${r} 0 0 1 ${r} -${r}z`;
  var ring = (cx, cy, r) => `M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 -${2 * r} 0`;
  var SHAPES = {
    records: ["M9 6h11", "M9 12h11", "M9 18h11", "M4.5 6h.01", "M4.5 12h.01", "M4.5 18h.01"],
    members: [ring(9, 8, 3.5), "M3 20c0-3.3 2.7-6 6-6s6 2.7 6 6", "M16 4.5a3.5 3.5 0 0 1 0 7", "M18 14c2 .6 3 2.8 3 6"],
    groups: [box(9, 3, 6, 5, 1.5), box(3, 16, 6, 5, 1.5), box(15, 16, 6, 5, 1.5), "M12 8v4", "M6 16v-4h12v4"],
    sun: [ring(12, 12, 4), "M12 2.5v2", "M12 19.5v2", "M2.5 12h2", "M19.5 12h2", "M5.3 5.3l1.4 1.4", "M17.3 17.3l1.4 1.4", "M5.3 18.7l1.4-1.4", "M17.3 6.7l1.4-1.4"],
    moon: ["M20 14.5A8.5 8.5 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"],
    menu: ["M4 7h16", "M4 12h16", "M4 17h16"],
    lock: [box(5, 11, 14, 10, 2), "M8 11V7.5a4 4 0 0 1 8 0V11"],
    "sign-out": ["M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4", "M15 8l4 4-4 4", "M19 12H9"],
    switch: ["M7 4L3 8l4 4", "M3 8h14", "M17 12l4 4-4 4", "M21 16H7"],
    download: ["M12 4v11", "M7 10l5 5 5-5", "M5 20h14"],
    copy: [box(8, 8, 13, 13, 2), "M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"],
    eye: ["M2.5 12s3.5-7 9.5-7 9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7z", ring(12, 12, 3)],
    "eye-off": [
      "M3 3l18 18",
      "M10.6 5.1A9.7 9.7 0 0 1 12 5c6 0 9.5 7 9.5 7a17 17 0 0 1-2.9 3.9",
      "M6.6 6.6C3.9 8.4 2.5 12 2.5 12s3.5 7 9.5 7a9.3 9.3 0 0 0 5.4-1.6",
      "M9.9 9.9a3 3 0 0 0 4.2 4.2"
    ],
    edit: ["M4 20h4l10.5-10.5a2.8 2.8 0 0 0-4-4L4 16z", "M13.5 6.5l4 4"],
    share: [ring(6, 12, 2.5), ring(18, 6, 2.5), ring(18, 18, 2.5), "M8.2 10.8l7.6-3.6", "M8.2 13.2l7.6 3.6"],
    trash: ["M4 7h16", "M9.5 7V4.5h5V7", "M6.5 7l1 13h9l1-13", "M10 11v5.5", "M14 11v5.5"],
    close: ["M6 6l12 12", "M18 6L6 18"],
    plus: ["M12 5v14", "M5 12h14"],
    search: [ring(11, 11, 6.5), "M20 20l-4.2-4.2"]
  };
  var isIconName = (name) => name !== void 0 && Object.hasOwn(SHAPES, name);
  function icon(name) {
    const svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("class", "icon");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");
    svg.setAttribute("fill", "none");
    svg.setAttribute("stroke", "currentColor");
    svg.setAttribute("stroke-width", "2");
    svg.setAttribute("stroke-linecap", "round");
    svg.setAttribute("stroke-linejoin", "round");
    for (const d of SHAPES[name]) {
      const path = document.createElementNS(SVG, "path");
      path.setAttribute("d", d);
      svg.append(path);
    }
    return svg;
  }

  // src/screen.ts
  var heading = (title) => el("h1", { tabindex: "-1", class: "view-title" }, title);
  var refusal = (words) => el("p", { class: "message bad", role: "alert" }, words);
  var breadcrumbs = (trail) => el(
    "nav",
    { class: "crumbs", "aria-label": "Breadcrumb" },
    el("ol", {}, ...trail.map((crumb) => el("li", {}, crumb.href === null ? el("span", { "aria-current": "page" }, crumb.label) : el("a", { href: crumb.href }, crumb.label))))
  );

  // src/records/types.ts
  function main(name, id, fields) {
    return { id, name, sections: [{ name: "", fields: fields.map(([label, type]) => ({ label, type })) }] };
  }
  var BUILT_IN_TYPES = [
    main("Login", "login", [
      ["Username", "text"],
      ["Password", "concealed"],
      ["Website", "url"],
      ["One-time password", "otp"]
    ]),
    main("Secure note", "secure-note", [["Note", "multiline"]]),
    main("SSH key", "ssh-key", [
      ["Key", "ssh-key"],
      ["Passphrase", "concealed"]
    ]),
    main("File", "file", [["File", "file"]]),
    main("Payment card", "payment-card", [
      ["Cardholder", "text"],
      ["Number", "concealed"],
      ["Expires", "month-year"],
      ["Security code", "concealed"],
      ["PIN", "concealed"]
    ]),
    main("Identity", "identity", [
      ["First name", "text"],
      ["Last name", "text"],
      ["Date of birth", "date"],
      ["Email", "email"],
      ["Phone", "phone"],
      ["Address", "multiline"]
    ]),
    main("Server", "server", [
      ["Address", "text"],
      ["Port", "number"],
      ["Username", "text"],
      ["Password", "concealed"]
    ]),
    main("Database", "database", [
      ["Kind", "text"],
      ["Host", "text"],
      ["Port", "number"],
      ["Database", "text"],
      ["Username", "text"],
      ["Password", "concealed"]
    ]),
    main("API credential", "api-credential", [
      ["Key id", "text"],
      ["Secret", "concealed"],
      ["Endpoint", "url"],
      ["Expires", "date"]
    ]),
    main("Wireless network", "wireless-network", [
      ["Network name", "text"],
      ["Password", "concealed"],
      ["Security", "text"]
    ]),
    main("Software license", "software-license", [
      ["Product", "text"],
      ["Version", "text"],
      ["License key", "concealed"],
      ["Licensed to", "text"],
      ["Email", "email"],
      ["Purchased", "date"],
      ["Expires", "date"]
    ])
  ];
  function recordFromTemplate(template, title) {
    return {
      v: 1,
      type: template.id,
      title,
      sections: template.sections.map((section) => ({
        name: section.name,
        fields: section.fields.map((field3) => emptyField(field3))
      }))
    };
  }
  function emptyField({ label, type }) {
    return { label, type, value: null };
  }

  // src/records/listing.ts
  function inScope(item, scope, me) {
    switch (scope.kind) {
      case "all":
        return true;
      case "mine":
        return item.row.author === me;
      case "group":
        return item.through.includes(scope.group);
    }
  }
  var KIND_ORDER = ["company", "department", "team"];
  function scopeGroups(items, groups) {
    const present = new Set(items.flatMap((item) => item.through));
    return groups.filter((group) => present.has(group.id)).sort((a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || a.name.localeCompare(b.name));
  }
  function matches(item, query, type) {
    if (type !== null && item.row.recordType !== type) {
      return false;
    }
    const wanted = query.trim().toLocaleLowerCase();
    if (wanted === "") {
      return true;
    }
    return "document" in item && item.document.title.toLocaleLowerCase().includes(wanted);
  }
  function unreadableWords(problem2) {
    switch (problem2) {
      case "unvouched-signer":
        return "It was written by someone your company's owner hasn't vouched for, so it isn't shown.";
      case "bad-signature":
        return "Its author's signature doesn't match, so it isn't shown. Ask its author to save it again.";
      case "unauthentic":
      case "key-mismatch":
      case "unknown-key":
        return "This page couldn't open it with your keys. Ask whoever shared it with you to share it again.";
      case "unvouched-recipient":
        return "It's shared through a group key your company's owner hasn't vouched for, so it isn't opened.";
      default:
        return "It's in a form this page can't read. Reload the page, and tell your admin if it stays.";
    }
  }
  var typeName = (id) => BUILT_IN_TYPES.find((template) => template.id === id)?.name ?? id;
  var COPY_CLEARS_AFTER_MS = 3e4;
  function copier(clipboard, clock, say) {
    let pending = null;
    const clear = async () => {
      pending = null;
      await clipboard.write("").catch(() => void 0);
      say("Clipboard cleared.");
    };
    return {
      async copy(value, label) {
        try {
          await clipboard.write(value);
        } catch {
          say("This browser didn't allow copying. Show the value and copy it yourself.");
          return;
        }
        if (pending !== null) {
          clock.clear(pending);
        }
        pending = clock.set(() => void clear(), COPY_CLEARS_AFTER_MS);
        say(`${label} copied. The clipboard clears in 30 seconds.`);
      },
      async clearNow() {
        if (pending === null) {
          return;
        }
        clock.clear(pending);
        await clear();
      }
    };
  }
  function findsRecords(press) {
    return press.key.toLowerCase() === "k" && (press.ctrlKey || press.metaKey) && !press.altKey && !press.shiftKey;
  }

  // src/chain-model.ts
  var OWNER_KEY_BYTES = 32;
  var isRecord5 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isText4 = (v) => typeof v === "string" && v !== "";
  function readChain(v) {
    if (!isRecord5(v) || !Array.isArray(v["statements"])) {
      return null;
    }
    const pinned = v["owner_signing"];
    const ownerSigning = pinned === null ? null : isText4(pinned) ? decode(pinned) : void 0;
    if (ownerSigning === void 0 || ownerSigning !== null && ownerSigning.length !== OWNER_KEY_BYTES) {
      return null;
    }
    const statements = [];
    for (const [index, item] of v["statements"].entries()) {
      if (!isRecord5(item) || item["seq"] !== index + 1 || !isText4(item["bytes"]) || !isText4(item["signature"])) {
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
    revision: `${PREFIX}revision`,
    /** SHA-256 prefix of the key fingerprint two people compare before one vouches for the other's key. */
    fingerprint: `${PREFIX}fingerprint`
  };
  var FORMAT_VERSION = 1;
  var ALGORITHM = {
    recordBody: 1,
    keyWrap: 2,
    memberKey: 3,
    signature: 4,
    file: 5
  };

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
  async function x25519PrivateFromBytes(raw, extractable) {
    const pkcs8 = concat(PKCS8_PREFIX_X25519, exactKey(raw));
    try {
      return await crypto.subtle.importKey("pkcs8", pkcs8, { name: "X25519" }, extractable, ["deriveBits"]);
    } finally {
      wipe(pkcs8);
    }
  }
  async function x25519PublicFromBytes(raw) {
    return crypto.subtle.importKey("raw", exactKey(raw), { name: "X25519" }, true, []);
  }
  async function ed25519PrivateFromSeed(seed, extractable) {
    const pkcs8 = concat(PKCS8_PREFIX_ED25519, exactKey(seed));
    try {
      return await crypto.subtle.importKey("pkcs8", pkcs8, { name: "Ed25519" }, extractable, ["sign"]);
    } finally {
      wipe(pkcs8);
    }
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
  async function x25519(privateKey, publicKey) {
    let shared;
    try {
      shared = new Uint8Array(await crypto.subtle.deriveBits({ name: "X25519", public: publicKey }, privateKey, 256));
    } catch {
      throw new VaultCryptoError("weak-shared-secret");
    }
    if (sameBytes(shared, new Uint8Array(KEY_BYTES))) {
      throw new VaultCryptoError("weak-shared-secret");
    }
    return shared;
  }

  // src/crypto/signatures.ts
  var SIGNATURE_BYTES = 64;
  async function sign(privateKey, message) {
    const signature = new Uint8Array(await crypto.subtle.sign("Ed25519", privateKey, message));
    return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.signature), signature);
  }
  async function verify(publicKey, message, stored) {
    const reader = new Reader(stored);
    if (reader.integer(1) !== FORMAT_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    if (reader.integer(1) !== ALGORITHM.signature) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    const signature = reader.take(SIGNATURE_BYTES);
    reader.end();
    let valid;
    try {
      valid = await crypto.subtle.verify("Ed25519", await ed25519PublicFromBytes(publicKey), signature, message);
    } catch {
      valid = false;
    }
    if (!valid) {
      throw new VaultCryptoError("bad-signature");
    }
  }

  // src/crypto/statements.ts
  var KEY_BYTES2 = 32;
  var DIGEST_BYTES = 32;
  var KINDS = ["member", "admin", "group", "revoke", "recovery"];
  var Vouched = class {
    #members;
    #groups;
    #recovery;
    constructor(members, groups, recovery) {
      this.#members = members;
      this.#groups = groups;
      this.#recovery = recovery;
    }
    recoveryKey() {
      return this.#recovery;
    }
    signingKeyOf(account) {
      return this.#members.get(account)?.signing;
    }
    agreementKeyOf(account) {
      return this.#members.get(account)?.agreement;
    }
    groupKeyOf(group) {
      return this.#groups.get(group);
    }
  };
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
    const text3 = (raw) => {
      try {
        return decoder.decode(raw);
      } catch {
        throw new VaultCryptoError("malformed");
      }
    };
    const decoded = {
      kind,
      company: text3(reader.field()),
      seq: reader.integer(8),
      prev: reader.take(DIGEST_BYTES),
      subject: text3(reader.field()),
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
  function hasShape(statement) {
    const agreement = statement.agreementPublic.length;
    const signing = statement.signingPublic.length;
    switch (statement.kind) {
      case "member":
        return agreement === KEY_BYTES2 && signing === KEY_BYTES2;
      case "group":
      case "recovery":
        return agreement === KEY_BYTES2 && signing === 0;
      case "admin":
      case "revoke":
        return agreement === 0 && signing === 0;
    }
  }
  async function verifyChain(company, ownerSigningPublic, chain) {
    if (chain.length === 0) {
      throw new VaultCryptoError("chain-broken");
    }
    const members = /* @__PURE__ */ new Map();
    const groups = /* @__PURE__ */ new Map();
    const adminKeys = /* @__PURE__ */ new Set();
    const forget2 = (subject) => {
      const keys = members.get(subject);
      if (keys !== void 0) {
        adminKeys.delete(hex(keys.signing));
      }
      return members.delete(subject);
    };
    let recovery;
    let expectedPrev = new Uint8Array(DIGEST_BYTES);
    let ownerAccount = "";
    for (const [index, statement] of chain.entries()) {
      const decoded = decode2(statement.bytes);
      await verify(decoded.signerPublic, statement.bytes, statement.signature);
      const placed = decoded.company === company && decoded.seq === index + 1 && sameBytes(decoded.prev, expectedPrev);
      if (!placed || !hasShape(decoded)) {
        throw new VaultCryptoError("chain-broken");
      }
      const byOwner = sameBytes(decoded.signerPublic, ownerSigningPublic);
      if (index === 0) {
        if (!byOwner || decoded.kind !== "member" || !sameBytes(decoded.signingPublic, ownerSigningPublic)) {
          throw new VaultCryptoError("chain-broken");
        }
        ownerAccount = decoded.subject;
      } else if (!byOwner && !adminKeys.has(hex(decoded.signerPublic))) {
        throw new VaultCryptoError("unvouched-signer");
      }
      switch (decoded.kind) {
        case "member":
          forget2(decoded.subject);
          members.set(decoded.subject, { agreement: decoded.agreementPublic, signing: decoded.signingPublic });
          break;
        case "admin": {
          const keys = members.get(decoded.subject);
          if (keys === void 0) {
            throw new VaultCryptoError("chain-broken");
          }
          adminKeys.add(hex(keys.signing));
          break;
        }
        case "group":
          groups.set(decoded.subject, decoded.agreementPublic);
          break;
        case "recovery":
          if (!byOwner) {
            throw new VaultCryptoError("unvouched-signer");
          }
          recovery = { keyId: decoded.subject, agreementPublic: decoded.agreementPublic };
          break;
        case "revoke":
          if (decoded.subject === ownerAccount || !forget2(decoded.subject) && !groups.delete(decoded.subject)) {
            throw new VaultCryptoError("chain-broken");
          }
          break;
      }
      expectedPrev = await digest(statement);
    }
    return new Vouched(members, groups, recovery);
  }

  // src/crypto/record.ts
  var KEY_ID_BYTES = 16;
  var SALT_BYTES = 32;
  var NONCE_BYTES = 12;
  var TAG_BYTES = 16;
  async function importRecordKey(raw) {
    if (raw.length !== 32) {
      throw new VaultCryptoError("malformed");
    }
    const key = await crypto.subtle.importKey("raw", raw, "HKDF", false, ["deriveKey"]);
    wipe(raw);
    return key;
  }
  function associatedData(context2, salt) {
    return concat(
      utf8(LABEL.record),
      field2(Uint8Array.of(FORMAT_VERSION)),
      field2(Uint8Array.of(ALGORITHM.recordBody)),
      field2(utf8(context2.company)),
      field2(utf8(context2.record)),
      field2(integer(context2.revision, 8)),
      field2(context2.keyId),
      field2(salt)
    );
  }
  async function contentKey(recordKey, salt, usage) {
    return crypto.subtle.deriveKey(
      { name: "HKDF", hash: "SHA-256", salt, info: utf8(LABEL.recordContent) },
      recordKey,
      { name: "AES-GCM", length: 256 },
      false,
      [usage]
    );
  }
  async function sealRevision(recordKey, context2, plaintext) {
    if (context2.keyId.length !== KEY_ID_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    const salt = random(SALT_BYTES);
    const nonce2 = random(NONCE_BYTES);
    const key = await contentKey(recordKey, salt, "encrypt");
    const sealed2 = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: nonce2, additionalData: associatedData(context2, salt) },
      key,
      plaintext
    );
    return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.recordBody), context2.keyId, salt, nonce2, new Uint8Array(sealed2));
  }
  async function openRevision(keys, context2, body) {
    const reader = new Reader(body);
    if (reader.integer(1) !== FORMAT_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    if (reader.integer(1) !== ALGORITHM.recordBody) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    const keyId = reader.take(KEY_ID_BYTES);
    const salt = reader.take(SALT_BYTES);
    const nonce2 = reader.take(NONCE_BYTES);
    const ciphertext = reader.rest();
    if (ciphertext.length < TAG_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    if (!sameBytes(keyId, context2.keyId)) {
      throw new VaultCryptoError("key-mismatch");
    }
    const recordKey = keys.get(hex(context2.keyId));
    if (recordKey === void 0) {
      throw new VaultCryptoError("unknown-key");
    }
    const key = await contentKey(recordKey, salt, "decrypt");
    try {
      return new Uint8Array(
        await crypto.subtle.decrypt(
          { name: "AES-GCM", iv: nonce2, additionalData: associatedData(context2, salt) },
          key,
          ciphertext
        )
      );
    } catch {
      throw new VaultCryptoError("unauthentic");
    }
  }
  function signedRevision(context2, body) {
    return concat(
      utf8(LABEL.revision),
      field2(utf8(context2.company)),
      field2(utf8(context2.record)),
      field2(integer(context2.revision, 8)),
      field2(body)
    );
  }
  async function signRevision(signingKey, context2, body) {
    return sign(signingKey, signedRevision(context2, body));
  }
  async function verifyRevision(vouched, author, context2, body, signature) {
    const signingKey = vouched.signingKeyOf(author);
    if (signingKey === void 0) {
      throw new VaultCryptoError("unvouched-signer");
    }
    await verify(signingKey, signedRevision(context2, body), signature);
  }

  // src/crypto/hkdf.ts
  var HASH_BYTES = 32;
  async function hmac(key, message) {
    const imported = await crypto.subtle.importKey("raw", key, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
    return new Uint8Array(await crypto.subtle.sign("HMAC", imported, message));
  }
  async function extract(salt, ikm) {
    return hmac(salt.length === 0 ? new Uint8Array(HASH_BYTES) : salt, ikm);
  }
  async function expand(prk, info, length) {
    if (!Number.isInteger(length) || length < 1 || length > 255 * HASH_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    const blocks = [];
    let previous = new Uint8Array(0);
    for (let counter = 1; blocks.length * HASH_BYTES < length; counter += 1) {
      previous = await hmac(prk, concat(previous, info, Uint8Array.of(counter)));
      blocks.push(previous);
    }
    return concat(...blocks).slice(0, length);
  }

  // src/crypto/hpke.ts
  var KEM_ID = 32;
  var KDF_ID = 1;
  var AEAD_ID = 2;
  var MODE_BASE = 0;
  var KEM_SUITE = concat(utf8("KEM"), integer(KEM_ID, 2));
  var HPKE_SUITE = concat(utf8("HPKE"), integer(KEM_ID, 2), integer(KDF_ID, 2), integer(AEAD_ID, 2));
  var VERSION_LABEL = utf8("HPKE-v1");
  var SECRET_BYTES = 32;
  var KEY_BYTES3 = 32;
  var NONCE_BYTES2 = 12;
  var HASH_BYTES2 = 32;
  async function labeledExtract(suite, salt, label, ikm) {
    return extract(salt, concat(VERSION_LABEL, suite, utf8(label), ikm));
  }
  async function labeledExpand(suite, prk, label, info, length) {
    return expand(prk, concat(integer(length, 2), VERSION_LABEL, suite, utf8(label), info), length);
  }
  async function sharedSecretOf(dh, enc, recipientPublic) {
    const prk = await labeledExtract(KEM_SUITE, new Uint8Array(0), "eae_prk", dh);
    const secret = await labeledExpand(KEM_SUITE, prk, "shared_secret", concat(enc, recipientPublic), SECRET_BYTES);
    wipe(prk, dh);
    return secret;
  }
  async function encapsulateWith(ephemeral, recipientPublic) {
    const dh = await x25519(ephemeral.privateKey, await x25519PublicFromBytes(recipientPublic));
    const enc = ephemeral.publicBytes.slice();
    return { sharedSecret: await sharedSecretOf(dh, enc, recipientPublic), enc };
  }
  async function keySchedule(sharedSecret, info) {
    const empty = new Uint8Array(0);
    const pskIdHash = await labeledExtract(HPKE_SUITE, empty, "psk_id_hash", empty);
    const infoHash = await labeledExtract(HPKE_SUITE, empty, "info_hash", info);
    const context2 = concat(Uint8Array.of(MODE_BASE), pskIdHash, infoHash);
    const secret = await labeledExtract(HPKE_SUITE, sharedSecret, "secret", empty);
    const key = await labeledExpand(HPKE_SUITE, secret, "key", context2, KEY_BYTES3);
    const baseNonce = await labeledExpand(HPKE_SUITE, secret, "base_nonce", context2, NONCE_BYTES2);
    const exporterSecret = await labeledExpand(HPKE_SUITE, secret, "exp", context2, HASH_BYTES2);
    wipe(secret);
    return { key, baseNonce, exporterSecret };
  }
  async function aeadKey(sharedSecret, info) {
    const schedule = await keySchedule(sharedSecret, info);
    const key = await crypto.subtle.importKey("raw", schedule.key, { name: "AES-GCM" }, false, ["encrypt", "decrypt"]);
    wipe(schedule.key, schedule.exporterSecret, sharedSecret);
    return { key, nonce: schedule.baseNonce };
  }
  async function sealWith(ephemeral, recipientPublic, info, aad, plaintext) {
    const { sharedSecret, enc } = await encapsulateWith(ephemeral, recipientPublic);
    const { key, nonce: nonce2 } = await aeadKey(sharedSecret, info);
    const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce2, additionalData: aad }, key, plaintext);
    return { enc, ciphertext: new Uint8Array(ciphertext) };
  }
  async function seal(recipientPublic, info, aad, plaintext) {
    const pair = await generatePair("X25519", false);
    return sealWith({ privateKey: pair.privateKey, publicBytes: await publicBytes(pair.publicKey) }, recipientPublic, info, aad, plaintext);
  }
  async function open(recipientPrivate, recipientPublic, enc, info, aad, ciphertext) {
    if (enc.length !== SECRET_BYTES) {
      throw new VaultCryptoError("malformed");
    }
    let dh;
    try {
      dh = await x25519(recipientPrivate, await x25519PublicFromBytes(enc));
    } catch {
      throw new VaultCryptoError("unauthentic");
    }
    const { key, nonce: nonce2 } = await aeadKey(await sharedSecretOf(dh, enc, recipientPublic), info);
    try {
      return new Uint8Array(await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce2, additionalData: aad }, key, ciphertext));
    } catch {
      throw new VaultCryptoError("unauthentic");
    }
  }

  // src/crypto/wraps.ts
  var ENC_BYTES = 32;
  var KEY_ID_BYTES2 = 16;
  function infoOf(context2) {
    if (context2.wrappedKeyId.length !== KEY_ID_BYTES2 || context2.recipientKeyId.length !== KEY_ID_BYTES2) {
      throw new VaultCryptoError("malformed");
    }
    return concat(
      utf8(LABEL.wrap),
      field2(utf8(context2.company)),
      field2(utf8(context2.wrappedKind)),
      field2(context2.wrappedKeyId),
      field2(utf8(context2.recipientKind)),
      field2(context2.recipientKeyId)
    );
  }
  async function wrapSecret(recipientPublic, context2, secret) {
    const sealed2 = await seal(recipientPublic, infoOf(context2), new Uint8Array(0), secret);
    return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.keyWrap), sealed2.enc, sealed2.ciphertext);
  }
  async function unwrapSecret(recipientPrivate, recipientPublic, context2, wrap) {
    const reader = new Reader(wrap);
    if (reader.integer(1) !== FORMAT_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    if (reader.integer(1) !== ALGORITHM.keyWrap) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    const enc = reader.take(ENC_BYTES);
    const ciphertext = reader.rest();
    return open(recipientPrivate, recipientPublic, enc, infoOf(context2), new Uint8Array(0), ciphertext);
  }

  // src/crypto/files.ts
  var FILE_CHUNK_BYTES = 64 * 1024;
  var FILE_MAX_BYTES = 256 * 1024 * 1024;
  var KEY_ID_BYTES3 = 16;
  var SALT_BYTES2 = 32;
  var PREFIX_BYTES = 7;
  var TAG_BYTES2 = 16;
  var SEALED_CHUNK_BYTES = FILE_CHUNK_BYTES + TAG_BYTES2;
  var MAX_CHUNKS = FILE_MAX_BYTES / FILE_CHUNK_BYTES;
  var FILE_KEY_BYTES = 32;
  var toHex = (bytes) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  function fromHex(text3, length) {
    if (!new RegExp(`^[0-9a-f]{${2 * length}}$`).test(text3)) {
      throw new VaultCryptoError("malformed");
    }
    return Uint8Array.from(text3.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
  }
  function newFileKey() {
    const key = random(FILE_KEY_BYTES);
    const made = { key: toHex(key), keyId: toHex(random(KEY_ID_BYTES3)) };
    wipe(key);
    return made;
  }
  async function importFileKey(key) {
    const raw = fromHex(key, FILE_KEY_BYTES);
    const imported = await crypto.subtle.importKey("raw", raw, "HKDF", false, ["deriveKey"]);
    wipe(raw);
    return imported;
  }
  var fileKeyId = (keyId) => fromHex(keyId, KEY_ID_BYTES3);
  function associatedData2(context2, salt, prefix) {
    return concat(
      utf8(LABEL.fileChunk),
      field2(utf8(context2.company)),
      field2(utf8(context2.record)),
      field2(utf8(context2.file)),
      field2(context2.keyId),
      field2(salt),
      field2(prefix)
    );
  }
  function nonce(prefix, chunk, last) {
    return concat(prefix, integer(chunk, 4), Uint8Array.of(last ? 1 : 0));
  }
  async function chunkKey(fileKey, salt, usage) {
    return crypto.subtle.deriveKey(
      { name: "HKDF", hash: "SHA-256", salt, info: utf8(LABEL.file) },
      fileKey,
      { name: "AES-GCM", length: 256 },
      false,
      [usage]
    );
  }
  var sealedSize = (length) => 2 + KEY_ID_BYTES3 + SALT_BYTES2 + PREFIX_BYTES + length + TAG_BYTES2 * Math.max(1, Math.ceil(length / FILE_CHUNK_BYTES));
  async function sealFile(fileKey, context2, data) {
    if (data.length > FILE_MAX_BYTES) {
      throw new VaultCryptoError("too-large");
    }
    if (context2.keyId.length !== KEY_ID_BYTES3) {
      throw new VaultCryptoError("malformed");
    }
    const salt = random(SALT_BYTES2);
    const prefix = random(PREFIX_BYTES);
    const key = await chunkKey(fileKey, salt, "encrypt");
    const additionalData = associatedData2(context2, salt, prefix);
    const count = Math.max(1, Math.ceil(data.length / FILE_CHUNK_BYTES));
    const sealed2 = [Uint8Array.of(FORMAT_VERSION, ALGORITHM.file), context2.keyId, salt, prefix];
    for (let chunk = 0; chunk < count; chunk += 1) {
      const plaintext = data.subarray(chunk * FILE_CHUNK_BYTES, (chunk + 1) * FILE_CHUNK_BYTES);
      const iv = nonce(prefix, chunk, chunk === count - 1);
      sealed2.push(new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv, additionalData }, key, plaintext)));
    }
    return concat(...sealed2);
  }
  async function openFile(keys, context2, sealed2) {
    const reader = new Reader(sealed2);
    if (reader.integer(1) !== FORMAT_VERSION) {
      throw new VaultCryptoError("unknown-version");
    }
    if (reader.integer(1) !== ALGORITHM.file) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    const keyId = reader.take(KEY_ID_BYTES3);
    const salt = reader.take(SALT_BYTES2);
    const prefix = reader.take(PREFIX_BYTES);
    const body = reader.rest();
    const count = Math.max(1, Math.ceil(body.length / SEALED_CHUNK_BYTES));
    const lastLength = body.length - (count - 1) * SEALED_CHUNK_BYTES;
    if (lastLength < TAG_BYTES2 || count > MAX_CHUNKS) {
      throw new VaultCryptoError("malformed");
    }
    if (!sameBytes(keyId, context2.keyId)) {
      throw new VaultCryptoError("key-mismatch");
    }
    const fileKey = keys.get(hex(context2.keyId));
    if (fileKey === void 0) {
      throw new VaultCryptoError("unknown-key");
    }
    const key = await chunkKey(fileKey, salt, "decrypt");
    const additionalData = associatedData2(context2, salt, prefix);
    const plain2 = [];
    for (let chunk = 0; chunk < count; chunk += 1) {
      const ciphertext = body.subarray(chunk * SEALED_CHUNK_BYTES, (chunk + 1) * SEALED_CHUNK_BYTES);
      const iv = nonce(prefix, chunk, chunk === count - 1);
      try {
        plain2.push(new Uint8Array(await crypto.subtle.decrypt({ name: "AES-GCM", iv, additionalData }, key, ciphertext)));
      } catch {
        throw new VaultCryptoError("unauthentic");
      }
    }
    return concat(...plain2);
  }

  // src/records/totp.ts
  var HASH = { SHA1: "SHA-1", SHA256: "SHA-256", SHA512: "SHA-512" };
  async function totpCode(params, unixSeconds) {
    if (!Number.isSafeInteger(params.period) || params.period < 1 || !Number.isSafeInteger(unixSeconds) || unixSeconds < 0) {
      throw new VaultCryptoError("malformed");
    }
    const key = await crypto.subtle.importKey("raw", params.secret, { name: "HMAC", hash: HASH[params.algorithm] }, false, [
      "sign"
    ]);
    const step = integer(Math.floor(unixSeconds / params.period), 8);
    const mac = new Uint8Array(await crypto.subtle.sign("HMAC", key, step));
    const offset = (mac[mac.length - 1] ?? 0) & 15;
    const binary = ((mac[offset] ?? 0) & 127) << 24 | (mac[offset + 1] ?? 0) << 16 | (mac[offset + 2] ?? 0) << 8 | (mac[offset + 3] ?? 0);
    return String(binary % 10 ** params.digits).padStart(params.digits, "0");
  }
  var ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  function base32Decode(text3) {
    const clean = text3.replace(/\s+/g, "").toUpperCase().replace(/=+$/, "");
    const out = [];
    let buffer = 0;
    let bits = 0;
    for (const character of clean) {
      const value = ALPHABET.indexOf(character);
      if (value < 0) {
        throw new VaultCryptoError("malformed");
      }
      buffer = buffer << 5 | value;
      bits += 5;
      if (bits >= 8) {
        out.push(buffer >> bits - 8 & 255);
        bits -= 8;
      }
      buffer &= (1 << bits) - 1;
    }
    if (bits >= 5 || buffer !== 0) {
      throw new VaultCryptoError("malformed");
    }
    return Uint8Array.from(out);
  }

  // src/records/fields.ts
  var FIELD_TYPES = [
    "text",
    "multiline",
    "concealed",
    "url",
    "email",
    "phone",
    "number",
    "date",
    "month-year",
    "boolean",
    "otp",
    "file",
    "security-question",
    "ssh-key"
  ];
  var MAX_TEXT = 65536;
  var SHAPES2 = {
    url: (text3) => URL.canParse(text3),
    email: (text3) => /^[^\s@]+@[^\s@]+$/.test(text3),
    phone: (text3) => /^[+0-9 ().-]{1,32}$/.test(text3),
    number: (text3) => /^-?\d+(\.\d+)?$/.test(text3),
    date: (text3) => {
      const match2 = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text3);
      if (match2 === null) {
        return false;
      }
      const [year, month, day] = [Number(match2[1]), Number(match2[2]), Number(match2[3])];
      const date = new Date(Date.UTC(year, month - 1, day));
      return date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
    },
    "month-year": (text3) => /^\d{4}-(0[1-9]|1[0-2])$/.test(text3)
  };
  function fail() {
    throw new VaultCryptoError("malformed");
  }
  function exactly(raw, keys) {
    if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
      return fail();
    }
    const present = Object.keys(raw);
    if (present.length !== keys.length || !keys.every((key) => present.includes(key))) {
      return fail();
    }
    return Object.fromEntries(Object.entries(raw));
  }
  function readText(raw) {
    if (typeof raw !== "string") {
      return fail();
    }
    if (raw.length > MAX_TEXT) {
      throw new VaultCryptoError("too-large");
    }
    return raw;
  }
  function readOtp(raw) {
    const value = exactly(raw, ["secret", "algorithm", "digits", "period"]);
    const secret = readText(value["secret"]);
    const { algorithm, digits, period } = value;
    if (base32Decode(secret).length === 0 || algorithm !== "SHA1" && algorithm !== "SHA256" && algorithm !== "SHA512" || digits !== 6 && digits !== 8 || typeof period !== "number" || !Number.isInteger(period) || period < 1 || period > 3600) {
      return fail();
    }
    return { secret, algorithm, digits, period };
  }
  var hexOf = (raw, bytes) => typeof raw === "string" && new RegExp(`^[0-9a-f]{${2 * bytes}}$`).test(raw) ? raw : fail();
  function readFile(raw) {
    const value = exactly(raw, ["id", "name", "size", "mediaType", "key", "keyId"]);
    const size = value["size"];
    if (typeof size !== "number" || !Number.isInteger(size) || size < 0 || size > FILE_MAX_BYTES) {
      return fail();
    }
    return {
      id: readText(value["id"]),
      name: readText(value["name"]),
      size,
      mediaType: readText(value["mediaType"]),
      key: hexOf(value["key"], 32),
      keyId: hexOf(value["keyId"], 16)
    };
  }
  function orNull(raw, read) {
    return raw === null ? null : read(raw);
  }
  function readShaped(type, raw) {
    const text3 = readText(raw);
    return text3 === "" || SHAPES2[type](text3) ? text3 : fail();
  }
  function readSecurityQuestion(raw) {
    const value = exactly(raw, ["question", "answer"]);
    return { question: readText(value["question"]), answer: readText(value["answer"]) };
  }
  function readSshKey(raw) {
    const value = exactly(raw, ["privateKey", "publicKey", "fingerprint"]);
    return {
      privateKey: readText(value["privateKey"]),
      publicKey: readText(value["publicKey"]),
      fingerprint: readText(value["fingerprint"])
    };
  }
  function isFieldType(raw) {
    return FIELD_TYPES.some((type) => type === raw);
  }
  function readField(raw) {
    const field3 = exactly(raw, ["label", "type", "value"]);
    const { type, value } = field3;
    if (!isFieldType(type)) {
      return fail();
    }
    const label = readText(field3["label"]);
    switch (type) {
      case "text":
      case "multiline":
      case "concealed":
        return { label, type, value: orNull(value, readText) };
      case "url":
      case "email":
      case "phone":
      case "number":
      case "date":
      case "month-year":
        return { label, type, value: orNull(value, (text3) => readShaped(type, text3)) };
      case "boolean":
        return { label, type, value: orNull(value, (flag) => typeof flag === "boolean" ? flag : fail()) };
      case "otp":
        return { label, type, value: orNull(value, readOtp) };
      case "file":
        return { label, type, value: orNull(value, readFile) };
      case "security-question":
        return { label, type, value: orNull(value, readSecurityQuestion) };
      case "ssh-key":
        return { label, type, value: orNull(value, readSshKey) };
    }
  }

  // src/records/document.ts
  var MAX_SECTIONS = 64;
  var MAX_FIELDS = 512;
  var TYPE_ID = /^[a-z0-9][a-z0-9:-]{0,63}$/;
  function fail2() {
    throw new VaultCryptoError("malformed");
  }
  function object(raw, keys) {
    if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
      return fail2();
    }
    const present = Object.keys(raw);
    if (present.length !== keys.length || !keys.every((key) => present.includes(key))) {
      return fail2();
    }
    return Object.fromEntries(Object.entries(raw));
  }
  function encodeRecord(document2) {
    return utf8(JSON.stringify(document2));
  }
  function decodeRecord(bytes) {
    let raw;
    try {
      raw = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
    } catch {
      return fail2();
    }
    if (typeof raw === "object" && raw !== null && "v" in raw && raw.v !== 1) {
      throw new VaultCryptoError("unknown-version");
    }
    const document2 = object(raw, ["v", "type", "title", "sections"]);
    const type = readText(document2["type"]);
    const sections = document2["sections"];
    if (!TYPE_ID.test(type) || !Array.isArray(sections)) {
      return fail2();
    }
    if (sections.length > MAX_SECTIONS) {
      throw new VaultCryptoError("too-large");
    }
    let fieldCount = 0;
    const read = sections.map((rawSection) => {
      const section = object(rawSection, ["name", "fields"]);
      const fields = section["fields"];
      if (!Array.isArray(fields)) {
        return fail2();
      }
      fieldCount += fields.length;
      if (fieldCount > MAX_FIELDS) {
        throw new VaultCryptoError("too-large");
      }
      return { name: readText(section["name"]), fields: fields.map(readField) };
    });
    return { v: 1, type, title: readText(document2["title"]), sections: read };
  }

  // src/records/open-record.ts
  async function unwrapped(company, vouched, recipient, opened) {
    const { row: row2 } = opened;
    const keyId = keyIdBytes(row2.keyId);
    if (keyId === null || opened.recipient.kind !== recipient.kind || opened.recipient.keyId !== hex(recipient.keyId)) {
      throw new VaultCryptoError("key-mismatch");
    }
    await verifyRevision(vouched, opened.revisedBy, { company, record: row2.id, revision: row2.revision }, opened.body, opened.signature);
    const raw = await unwrapSecret(
      recipient.agreementPrivate,
      recipient.agreementPublic,
      { company, wrappedKind: "record", wrappedKeyId: keyId, recipientKind: recipient.kind, recipientKeyId: recipient.keyId },
      opened.wrap
    );
    return { raw, keyId };
  }
  async function recordKeyOf(company, vouched, recipient, opened) {
    return (await unwrapped(company, vouched, recipient, opened)).raw;
  }
  async function openRecord(company, vouched, recipient, opened) {
    const { row: row2 } = opened;
    const { raw, keyId } = await unwrapped(company, vouched, recipient, opened);
    const place = { company, record: row2.id, revision: row2.revision };
    const recordKey = await importRecordKey(raw);
    const plaintext = await openRevision(/* @__PURE__ */ new Map([[row2.keyId, recordKey]]), { ...place, keyId }, opened.body);
    try {
      return decodeRecord(plaintext);
    } finally {
      wipe(plaintext);
    }
  }

  // src/records/recipient-keys.ts
  var memberKey = (keys) => ({
    kind: "member",
    keyId: keys.keyId,
    agreementPrivate: keys.agreementPrivate,
    agreementPublic: keys.agreementPublic
  });
  async function groupKey(company, vouched, member, group, listed) {
    const vouchedPublic = vouched.groupKeyOf(group);
    if (vouchedPublic === void 0 || !sameBytes(vouchedPublic, listed.public)) {
      throw new VaultCryptoError("unvouched-recipient");
    }
    if (listed.wrap === null) {
      throw new VaultCryptoError("unknown-key");
    }
    const raw = await unwrapSecret(
      member.agreementPrivate,
      member.agreementPublic,
      { company, wrappedKind: "group", wrappedKeyId: listed.keyId, recipientKind: "member", recipientKeyId: member.keyId },
      listed.wrap
    );
    try {
      return { kind: "group", keyId: listed.keyId, agreementPrivate: await x25519PrivateFromBytes(raw, false), agreementPublic: listed.public };
    } finally {
      wipe(raw);
    }
  }

  // src/records/load.ts
  var companyPath = (held2) => `/api/companies/${encodeURIComponent(held2.company)}`;
  async function vouchedChain(held2) {
    const chain = await call("GET", `${companyPath(held2)}/statements`, readChain);
    if (!chain.ok) {
      return chain;
    }
    try {
      return { ok: true, vouched: await verifyChain(held2.company, held2.keys.ownerSigningPublic, chain.value.statements) };
    } catch (problem2) {
      return { ok: false, problem: problem2 instanceof VaultCryptoError ? problem2 : new VaultCryptoError("chain-broken") };
    }
  }
  var Opener = class {
    #held;
    #vouched;
    #groups = null;
    constructor(held2, vouched) {
      this.#held = held2;
      this.#vouched = vouched;
    }
    /** The record opened, or the reason it would not open. */
    async open(opened) {
      try {
        const key = await this.keyFor(opened);
        return { row: opened.row, through: opened.through, document: await openRecord(this.#held.company, this.#vouched, key, opened) };
      } catch (problem2) {
        return { row: opened.row, through: opened.through, problem: problem2 instanceof VaultCryptoError ? problem2.reason : "malformed" };
      }
    }
    /** The person's own key or the group key `opened`'s wrap is kept for; a share re-opens that wrap with it. */
    async keyFor(opened) {
      if (opened.recipient.kind === "member") {
        return memberKey(this.#held.keys);
      }
      this.#groups ??= this.#groupKeys();
      const found = (await this.#groups).get(opened.recipient.keyId);
      if (found === void 0) {
        throw new VaultCryptoError("unknown-key");
      }
      return groupKey(this.#held.company, this.#vouched, this.#held.keys, found.group, found.listed);
    }
    /** Every group key the company lists, by key id; a group the person cannot read is left out. */
    async #groupKeys() {
      const ids = await call("GET", `${companyPath(this.#held)}/groups`, readGroupIds);
      const keys = /* @__PURE__ */ new Map();
      if (!ids.ok) {
        return keys;
      }
      for (const group of groupsToOpen(this.#held.company, ids.value)) {
        const listed = await call("GET", `${companyPath(this.#held)}/groups/${encodeURIComponent(group)}/key`, readGroupKey);
        if (listed.ok) {
          keys.set(hex(listed.value.keyId), { group, listed: listed.value });
        }
      }
      return keys;
    }
  };

  // src/records/list-view.ts
  var when = new Intl.DateTimeFormat(void 0, { dateStyle: "medium", timeStyle: "short" });
  var loadFailure = (failure) => failure.code === "network" ? "The vault didn't answer. Check your connection, then try again." : "The vault couldn't load your records. Try again in a minute.";
  var SKELETON_AFTER_MS = 200;
  var CHAIN_REFUSED = "Your company's list of who may read what doesn't check out against the owner key your keys trust, so nothing is opened. Tell your company's owner.";
  var sameScope = (a, b) => a.kind === b.kind && (a.kind !== "group" || b.kind === "group" && a.group === b.group);
  function row(item, href) {
    const type = el("td", { class: "muted" }, typeName(item.row.recordType));
    const changed = el("td", { class: "muted when" }, when.format(new Date(item.row.updated * 1e3)));
    if (!("document" in item)) {
      return el(
        "tr",
        { class: "unreadable" },
        el("td", {}, el("span", { class: "record-title" }, "Record not shown"), el("p", { class: "message bad" }, unreadableWords(item.problem))),
        type,
        changed
      );
    }
    const title = item.document.title === "" ? "Untitled" : item.document.title;
    return el("tr", {}, el("td", {}, el("a", { class: "record-title", href: href(item.row.id) }, title)), type, changed);
  }
  async function drawList(main2, held2, href, newHref, stops2) {
    const title = heading("Records");
    const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Opening your records."));
    const head = el("div", { class: "page-head" }, title, el("a", { class: "button primary", href: newHref }, "New record"));
    fill(main2, head, said);
    const skeleton = el("ul", { class: "records skeleton", "aria-hidden": "true" }, ...[1, 2, 3].map(() => el("li", { class: "record" }, el("span", { class: "bone" }))));
    const slow = window.setTimeout(() => main2.append(skeleton), SKELETON_AFTER_MS);
    const loaded = () => {
      window.clearTimeout(slow);
      skeleton.remove();
    };
    stops2.push(loaded);
    const [chain, listedGroups] = await Promise.all([vouchedChain(held2), call("GET", `${companyPath(held2)}/groups`, readGroupList)]);
    const groups = listedGroups.ok ? listedGroups.value : [];
    if (!chain.ok) {
      loaded();
      fill(said, refusal("problem" in chain ? CHAIN_REFUSED : loadFailure(chain)));
      return;
    }
    const opener = new Opener(held2, chain.vouched);
    const items = [];
    let next = null;
    let type = null;
    let scope = { kind: "all" };
    const search = el("input", { id: "record-filter", type: "search", autocomplete: "off", "aria-keyshortcuts": "Control+K Meta+K" });
    const toFilter = (event) => {
      if (findsRecords(event) && !filter.hidden) {
        event.preventDefault();
        search.focus();
      }
    };
    document.addEventListener("keydown", toFilter);
    stops2.push(() => document.removeEventListener("keydown", toFilter));
    const scopes = el("div", { class: "chips", role: "group", "aria-label": "Show" });
    const types = el("div", { class: "chips", role: "group", "aria-label": "Type" });
    const rows = el("tbody");
    const list = el(
      "table",
      { class: "records" },
      el("caption", { class: "sr-only" }, "Records"),
      el("thead", {}, el("tr", {}, el("th", { scope: "col" }, "Title"), el("th", { scope: "col" }, "Type"), el("th", { scope: "col" }, "Changed"))),
      rows
    );
    const more = el("button", { type: "button", class: "quiet" }, "Load more");
    const tail = el("div", {});
    const filter = el("div", { class: "field search" }, el("label", { for: "record-filter" }, "Find a record by title"), el("span", { class: "search-box" }, icon("search"), search));
    const render = () => {
      filter.hidden = items.length === 0;
      types.hidden = items.length === 0;
      scopes.hidden = items.length === 0;
      const offered = [
        [{ kind: "all" }, "All"],
        [{ kind: "mine" }, "Mine"],
        ...scopeGroups(items, groups).map((group) => [{ kind: "group", group: group.id }, group.name])
      ];
      fill(
        scopes,
        ...offered.map(([each, words]) => {
          const chip = el("button", { type: "button", class: "chip", "aria-pressed": String(sameScope(each, scope)) }, words);
          chip.addEventListener("click", () => {
            scope = each;
            render();
          });
          return chip;
        })
      );
      const present = [...new Set(items.map((item) => item.row.recordType))];
      fill(
        types,
        ...[null, ...present].map((id) => {
          const chip = el("button", { type: "button", class: "chip", "aria-pressed": String(type === id) }, id === null ? "All types" : typeName(id));
          chip.addEventListener("click", () => {
            type = id;
            render();
          });
          return chip;
        })
      );
      const kept = items.filter((item) => inScope(item, scope, held2.account) && matches(item, search.value, type));
      fill(rows, ...kept.map((item) => row(item, href)));
      list.hidden = kept.length === 0;
      if (items.length === 0) {
        fill(said, el("p", {}, "No records yet. Records you make, and records others share with you, appear here."));
      } else if (kept.length === 0) {
        const clear = el("button", { type: "button", class: "quiet" }, "Clear the filter");
        clear.addEventListener("click", () => {
          search.value = "";
          type = null;
          scope = { kind: "all" };
          render();
          search.focus();
        });
        const query = search.value.trim();
        fill(said, el("p", {}, query === "" ? "No records match these filters." : `No records match "${query}".`), clear);
      } else {
        fill(said);
      }
      fill(tail, next === null ? null : more);
    };
    const load = async () => {
      more.disabled = true;
      const page = await call("GET", openedPagePath(held2.company, next), readOpenedPage);
      more.disabled = false;
      if (!page.ok) {
        fill(said, refusal(loadFailure(page)));
        return false;
      }
      for (const opened of page.value.records) {
        items.push(await opener.open(opened));
      }
      next = page.value.next;
      return true;
    };
    const first = await load();
    loaded();
    if (!first) {
      return;
    }
    search.addEventListener("input", render);
    more.addEventListener("click", () => void load().then((loaded2) => loaded2 && render()));
    fill(main2, head, el("div", { class: "toolbar" }, filter, el("div", { class: "filters" }, scopes, types)), said, list, tail);
    render();
    title.focus();
  }

  // src/crypto/recipients.ts
  function vouchedKey(chain, recipient) {
    switch (recipient.kind) {
      case "member":
        return chain.agreementKeyOf(recipient.account);
      case "group":
        return chain.groupKeyOf(recipient.group);
      case "recovery":
        return chain.recoveryKey()?.agreementPublic;
    }
  }
  async function wrapToVouched(chain, recipient, listed, context2, secret) {
    if (context2.recipientKind !== recipient.kind) {
      throw new VaultCryptoError("malformed");
    }
    const vouched = vouchedKey(chain, recipient);
    if (vouched === void 0 || !sameBytes(vouched, listed)) {
      throw new VaultCryptoError("unvouched-recipient");
    }
    return wrapSecret(vouched, context2, secret);
  }

  // src/records/share-record.ts
  async function shareBody(company, vouched, reader, opened, to, right) {
    const raw = await recordKeyOf(company, vouched, reader, opened);
    try {
      const wrappedKeyId = keyIdBytes(opened.row.keyId);
      if (wrappedKeyId === null) {
        throw new VaultCryptoError("key-mismatch");
      }
      const context2 = { company, wrappedKind: "record", wrappedKeyId, recipientKind: to.recipient.kind, recipientKeyId: to.keyId };
      const wrap = await wrapToVouched(vouched, to.recipient, to.listed, context2, raw);
      const target = to.recipient.kind === "member" ? { member: to.recipient.account } : { group: to.recipient.group };
      return { ...target, right, key_id: hex(to.keyId), wrap: encode(wrap) };
    } finally {
      wipe(raw);
    }
  }

  // src/records/share-words.ts
  var REOPEN = "This record or its sharing changed while you were deciding. Open it again, then share it.";
  function shareFailure(status, code, fallback) {
    if (status === 403) {
      return "You can read this record but not share it.";
    }
    if (status !== 409) {
      return fallback;
    }
    switch (code) {
      case "already_shared":
        return "This record is shared with them already.";
      case "no_key":
        return "They have no key yet, so nothing can be shared with them. Ask them to set their unlock password first.";
      case "unknown_key":
        return "Their key changed while you were deciding. Open the record again, then share it.";
      case "stale_revision":
      case "stale_shares":
        return REOPEN;
      default:
        return fallback;
    }
  }
  function sealFailure(reason) {
    return reason === "unvouched-recipient" ? "Their key is not vouched for in the company yet, so nothing can be shared with them. An admin vouches for new keys." : "This record could not be opened to share it. Open it again, then share it.";
  }
  function removeShareFailure(status, _code, fallback) {
    if (status === 403) {
      return "You can read this record but not change who it is shared with.";
    }
    if (status === 404) {
      return "The record is no longer shared with them, or it is gone.";
    }
    return fallback;
  }

  // src/records/shares-model.ts
  var isRecord6 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isId3 = (v) => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
  function readShares(v) {
    if (!isRecord6(v) || !isRecord6(v["author"]) || !Array.isArray(v["shares"])) {
      return null;
    }
    const { account, key_id: authorKey } = v["author"];
    if (!isId3(account) || authorKey !== null && !isId3(authorKey)) {
      return null;
    }
    const shares = v["shares"].map((share2) => {
      if (!isRecord6(share2) || !isId3(share2["target"]) || !isId3(share2["key_id"])) {
        return null;
      }
      const { kind, right } = share2;
      return (kind === "member" || kind === "group") && (right === "read" || right === "edit") ? { kind, target: share2["target"], right, keyId: share2["key_id"] } : null;
    });
    return shares.every((share2) => share2 !== null) ? { author: { account, keyId: authorKey }, shares } : null;
  }
  function readDirectory(v) {
    if (!isRecord6(v) || !Array.isArray(v["keys"])) {
      return null;
    }
    const entries = v["keys"].map((entry) => {
      if (!isRecord6(entry) || !isId3(entry["account"]) || !isId3(entry["key_id"]) || typeof entry["agreement_public"] !== "string" || typeof entry["signing_public"] !== "string") {
        return null;
      }
      const agreementPublic = decode(entry["agreement_public"]);
      const signingPublic = decode(entry["signing_public"]);
      return agreementPublic?.length === 32 && signingPublic?.length === 32 ? [entry["account"], { keyId: entry["key_id"], agreementPublic, signingPublic }] : null;
    });
    return entries.every((entry) => entry !== null) ? new Map(entries) : null;
  }
  function revisionRecipients(listed, directory, groups, own) {
    const wanted = [
      ...listed.author.keyId === null ? [] : [{ kind: "member", target: listed.author.account, right: "edit", keyId: listed.author.keyId }],
      ...listed.shares
    ];
    const recipients = [];
    for (const share2 of wanted.filter((each) => !(each.kind === "member" && each.keyId === own))) {
      const keyId = keyIdBytes(share2.keyId);
      const member = share2.kind === "member" ? directory.get(share2.target) : void 0;
      const key = share2.kind === "member" ? member?.keyId === share2.keyId ? member.agreementPublic : void 0 : groups.get(share2.target);
      if (keyId === null || key === void 0) {
        return null;
      }
      recipients.push({
        recipient: share2.kind === "member" ? { kind: "member", account: share2.target } : { kind: "group", group: share2.target },
        keyId,
        listed: key
      });
    }
    return recipients;
  }

  // src/records/sharing.ts
  var failed = (answer) => ({ ok: false, words: loadFailure(answer) });
  async function loadSharing(held2, id) {
    const path = companyPath(held2);
    const [listed, members, groups, directory] = await Promise.all([
      call("GET", `${path}/records/${encodeURIComponent(id)}/shares`, readShares),
      call("GET", `${path}/members`, readMemberList),
      call("GET", `${path}/groups`, readGroupList),
      call("GET", `${path}/keys`, readDirectory)
    ]);
    if (!listed.ok) {
      return listed.status === 403 ? null : failed(listed);
    }
    if (!members.ok) {
      return failed(members);
    }
    if (!groups.ok) {
      return failed(groups);
    }
    if (!directory.ok) {
      return failed(directory);
    }
    const all = companyAndGroups({ id: held2.company, name: held2.companyName }, groups.value);
    const groupKeys = await loadGroupKeys(held2, all);
    if (!groupKeys.ok) {
      return failed(groupKeys);
    }
    const { author, shares } = listed.value;
    return {
      ok: true,
      rows: sharedRows(shares, members.value, all),
      targets: shareTargets(members.value, all, directory.value, groupKeys.keys, author.account, shares.map((share2) => share2.target))
    };
  }
  async function loadGroupKeys(held2, groups) {
    const path = companyPath(held2);
    const answers = await Promise.all(
      groups.map((group) => call("GET", `${path}/groups/${encodeURIComponent(group.id)}/key`, readGroupKey).then((key) => [group.id, key]))
    );
    const keys = /* @__PURE__ */ new Map();
    for (const [group, key] of answers) {
      if (key.ok) {
        keys.set(group, { keyId: hex(key.value.keyId), public: key.value.public });
      } else if (key.status !== 404) {
        return key;
      }
    }
    return { ok: true, keys };
  }

  // src/records/audience-view.ts
  async function loadAudience(held2) {
    const groups = await call("GET", `${companyPath(held2)}/groups`, readGroupList);
    if (!groups.ok) {
      return { ok: false, words: loadFailure(groups) };
    }
    const company = { id: held2.company, name: held2.companyName };
    const keys = await loadGroupKeys(held2, [{ id: company.id, kind: "company", name: company.name }, ...groups.value]);
    return keys.ok ? { ok: true, choices: audienceChoices(company, groups.value, keys.keys) } : { ok: false, words: loadFailure(keys) };
  }
  function audiencePicker(choices) {
    let chosen = null;
    const row2 = (id, label, detail, choose, checked, disabled) => {
      const radio = el("input", { id, type: "radio", name: "record-audience", ...checked ? { checked: "" } : {} });
      radio.disabled = disabled;
      radio.addEventListener("change", choose);
      return el("div", { class: "choice" }, radio, el("label", { for: id }, label, el("span", { class: "detail" }, detail)));
    };
    const rows = [
      row2("audience-only-me", "Only you", "No one else until you share it", () => chosen = null, true, false),
      ...choices.map(
        (choice) => row2(
          `audience-${choice.id}`,
          choice.label,
          choice.recipient === null ? `${choice.detail} · ${choice.missing}` : choice.detail,
          () => chosen = choice,
          false,
          choice.recipient === null
        )
      )
    ];
    const section = el("fieldset", { class: "record-section" }, el("legend", {}, "Who can open it"), el("div", { class: "targets" }, ...rows));
    return { section, chosen: () => chosen };
  }
  async function shareCreated(held2, id, to) {
    const path = companyPath(held2);
    const [chain, answer] = await Promise.all([vouchedChain(held2), call("GET", `${path}/records/${encodeURIComponent(id)}`, readOpened)]);
    if (!chain.ok) {
      return "problem" in chain ? CHAIN_REFUSED : loadFailure(chain);
    }
    if (!answer.ok) {
      return loadFailure(answer);
    }
    let body;
    try {
      const opener = new Opener(held2, chain.vouched);
      body = await shareBody(held2.company, chain.vouched, await opener.keyFor(answer.value), answer.value, to, "read");
    } catch (problem2) {
      return sealFailure(problem2 instanceof VaultCryptoError ? problem2.reason : "malformed");
    }
    const shared = await call("POST", `${path}/records/${encodeURIComponent(id)}/shares`, ignored, { body });
    return shared.ok ? null : shareFailure(shared.status, shared.code, loadFailure(shared));
  }

  // src/records/custom-fields.ts
  var CUSTOM_SECTION = "Custom fields";
  var MAX_LABEL = 64;
  var isCustom = (field3) => field3.type === "text" || field3.type === "concealed" || field3.type === "ssh-key" || field3.type === "file";
  function splitCustom(document2) {
    const last = document2.sections.at(-1);
    if (last === void 0 || last.name !== CUSTOM_SECTION || !last.fields.every(isCustom)) {
      return { base: document2, custom: [] };
    }
    return { base: { ...document2, sections: document2.sections.slice(0, -1) }, custom: last.fields.filter(isCustom) };
  }
  function joinCustom(base, custom) {
    return custom.length === 0 ? base : { ...base, sections: [...base.sections, { name: CUSTOM_SECTION, fields: [...custom] }] };
  }
  function customLabelProblem(label, taken) {
    const name = label.trim();
    if (name === "") {
      return "Give the field a name.";
    }
    if (name.length > MAX_LABEL) {
      return "Keep the name to 64 characters.";
    }
    const folded = name.toLocaleLowerCase();
    return taken.some((other) => other.trim().toLocaleLowerCase() === folded) ? "This record already has a field called that." : null;
  }

  // src/icon-action.ts
  var OPEN_DELAY_MS = 500;
  var INSTANT_WINDOW_MS = 400;
  function iconAction(name, tip, label, tone = "quiet") {
    return el("button", { type: "button", class: `icon-action ${tone}`, "aria-label": label, "data-tip": tip }, icon(name));
  }
  function installTips() {
    const tip = el("div", { class: "tip", role: "tooltip", "aria-hidden": "true" });
    document.body.append(tip);
    let anchor = null;
    let timer;
    let closedAt = Number.NEGATIVE_INFINITY;
    const place = (target2) => {
      const box2 = target2.getBoundingClientRect();
      const own = tip.getBoundingClientRect();
      const above = box2.top - own.height - 6;
      const below = above < 4;
      const left = Math.min(Math.max(4, box2.left + box2.width / 2 - own.width / 2), window.innerWidth - own.width - 4);
      tip.style.left = `${left}px`;
      tip.style.top = `${below ? box2.bottom + 6 : above}px`;
      tip.dataset["side"] = below ? "below" : "above";
    };
    const show = (target2, instant) => {
      anchor = target2;
      tip.textContent = target2.dataset["tip"] ?? "";
      tip.toggleAttribute("data-instant", instant);
      tip.dataset["open"] = "";
      place(target2);
    };
    const hide = () => {
      window.clearTimeout(timer);
      if (anchor !== null) {
        closedAt = performance.now();
      }
      anchor = null;
      delete tip.dataset["open"];
    };
    const target = (event) => event.target instanceof Element ? event.target.closest("[data-tip]") : null;
    document.addEventListener("pointerover", (event) => {
      const over = target(event);
      if (over === null || over === anchor) {
        return;
      }
      hide();
      const instant = performance.now() - closedAt < INSTANT_WINDOW_MS;
      timer = window.setTimeout(() => show(over, instant), instant ? 0 : OPEN_DELAY_MS);
    });
    document.addEventListener("pointerout", (event) => {
      if (target(event) !== null) {
        hide();
      }
    });
    document.addEventListener("focusin", (event) => {
      const focused = target(event);
      if (focused !== null && focused.matches(":focus-visible")) {
        hide();
        show(focused, true);
      }
    });
    document.addEventListener("focusout", hide);
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        hide();
      }
    });
    window.addEventListener("scroll", hide, { capture: true, passive: true });
  }

  // src/records/custom-fields-view.ts
  var KINDS2 = [
    { type: "text", words: "Text" },
    { type: "concealed", words: "Secret" },
    { type: "ssh-key", words: "SSH key" },
    { type: "file", words: "File" }
  ];
  function customEditor(initial, controlFor, taken) {
    let made = 0;
    const entries = [];
    const list = el("div", { class: "custom-fields" });
    const append = (field3) => {
      made += 1;
      const control2 = controlFor(field3, `custom-${made}`);
      const remove = iconAction("trash", "Remove field", `Remove the field ${field3.label}`, "danger");
      const input = control2.row.querySelector("input");
      if (input === null) {
        (control2.row.querySelector(".input-line") ?? control2.row).append(remove);
      } else {
        const line = el("div", { class: "input-line" });
        input.replaceWith(line);
        line.append(input, remove);
      }
      const row2 = el("div", { class: "custom-row" }, control2.row);
      const entry = { field: field3, control: control2, row: row2 };
      remove.addEventListener("click", () => {
        entries.splice(entries.indexOf(entry), 1);
        row2.remove();
        opener.focus();
      });
      entries.push(entry);
      list.append(row2);
      return row2;
    };
    initial.forEach(append);
    const name = el("input", { id: "custom-name", type: "text", autocomplete: "off", maxlength: "64" });
    const hint = el("p", { id: "custom-name-hint", class: "message bad" });
    hint.hidden = true;
    name.setAttribute("aria-describedby", hint.id);
    let kind = "text";
    const chips = KINDS2.map(({ type, words }) => {
      const chip = el("button", { type: "button", class: "chip", "aria-pressed": String(type === kind) }, words);
      chip.addEventListener("click", () => {
        kind = type;
        chips.forEach((other, at2) => other.setAttribute("aria-pressed", String(KINDS2[at2]?.type === kind)));
      });
      return chip;
    });
    const add = el("button", { type: "button", class: "primary" }, "Add field");
    const cancel = el("button", { type: "button", class: "quiet" }, "Cancel");
    const panel = el(
      "div",
      { class: "custom-add" },
      el("div", { class: "field" }, el("label", { for: name.id }, "Field name"), name, hint),
      el("div", { class: "chips", role: "group", "aria-label": "Kind of field" }, ...chips),
      el("div", { class: "actions" }, add, cancel)
    );
    panel.hidden = true;
    const opener = el("button", { type: "button", class: "quiet add-field", "aria-expanded": "false" }, icon("plus"), el("span", {}, "Add a field"));
    const close = () => {
      panel.hidden = true;
      opener.hidden = false;
      opener.setAttribute("aria-expanded", "false");
    };
    opener.addEventListener("click", () => {
      name.value = "";
      hint.hidden = true;
      name.removeAttribute("aria-invalid");
      panel.hidden = false;
      opener.hidden = true;
      opener.setAttribute("aria-expanded", "true");
      name.focus();
    });
    const submit = () => {
      const problem2 = customLabelProblem(name.value, [...taken, ...entries.map((entry) => entry.field.label)]);
      hint.hidden = problem2 === null;
      fill(hint, problem2 ?? "");
      if (problem2 !== null) {
        name.setAttribute("aria-invalid", "true");
        name.focus();
        return;
      }
      const row2 = append({ label: name.value.trim(), type: kind, value: null });
      close();
      row2.querySelector("input, button")?.focus();
    };
    add.addEventListener("click", submit);
    cancel.addEventListener("click", () => {
      close();
      opener.focus();
    });
    name.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        submit();
      } else if (event.key === "Escape") {
        event.preventDefault();
        close();
        opener.focus();
      }
    });
    const section = el("fieldset", { class: "record-section" }, el("legend", {}, CUSTOM_SECTION), list, opener, panel);
    return { section, current: () => entries.map(({ field: field3, control: control2 }) => ({ field: field3, control: control2 })) };
  }

  // src/records/crumbs.ts
  var RECORDS = "Records";
  var named = (title) => title === null ? "Record" : title === "" ? "Untitled" : title;
  var recordCrumbs = (records, title) => [
    { label: RECORDS, href: records },
    { label: named(title), href: null }
  ];
  var editCrumbs = (records, record, title) => [
    { label: RECORDS, href: records },
    { label: named(title), href: record },
    { label: "Edit", href: null }
  ];
  function newCrumbs(records, chooser, typeName2) {
    const top = { label: RECORDS, href: records };
    return typeName2 === null ? [top, { label: "New record", href: null }] : [top, { label: "New record", href: chooser }, { label: typeName2, href: null }];
  }

  // src/records/editing.ts
  var fieldAt = (section, field3) => `${section}.${field3}`;
  var FORMAT = {
    url: "Use a full address starting with https://.",
    email: "Use an address like name@example.com.",
    phone: "Use digits, spaces and + ( ) - only.",
    number: "Use digits only.",
    date: "Use a date like 2027-02-28.",
    "month-year": "Use a month like 2028-11.",
    otp: "Paste the setup key exactly as the issuer showed it."
  };
  function problem(type, refused) {
    if (refused instanceof VaultCryptoError && refused.reason === "too-large") {
      return "This value is too long to keep.";
    }
    return FORMAT[type] ?? "This value isn't in a form the record can keep.";
  }
  function edited(document2, title, values) {
    const problems = /* @__PURE__ */ new Map();
    const named2 = title.trim();
    if (named2 === "") {
      problems.set("title", "Give the record a title, so you can find it again.");
    }
    const sections = document2.sections.map((section, sectionIndex) => ({
      name: section.name,
      fields: section.fields.map((field3, fieldIndex) => {
        const at2 = fieldAt(sectionIndex, fieldIndex);
        const typed = values.has(at2) ? values.get(at2) : field3.value;
        try {
          return readField({ label: field3.label, type: field3.type, value: typed === "" ? null : typed });
        } catch (refused) {
          problems.set(at2, problem(field3.type, refused));
          return field3;
        }
      })
    }));
    return problems.size === 0 ? { ok: true, document: { ...document2, title: named2, sections } } : { ok: false, problems };
  }

  // src/records/save-record.ts
  var ID_BYTES = 16;
  var RECORD_KEY_BYTES = 32;
  async function sealed(company, keys, id, revision, document2, vouched, others) {
    const keyId = random(ID_BYTES);
    const raw = random(RECORD_KEY_BYTES);
    const context2 = (recipientKind, recipientKeyId) => ({ company, wrappedKind: "record", wrappedKeyId: keyId, recipientKind, recipientKeyId });
    const copy2 = raw.slice();
    let wrap;
    const wraps = [];
    try {
      wrap = await wrapSecret(keys.agreementPublic, context2("member", keys.keyId), copy2);
      for (const other of others) {
        if (vouched === null) {
          throw new RangeError("a revision shared with others needs the verified chain");
        }
        const made = await wrapToVouched(vouched, other.recipient, other.listed, context2(other.recipient.kind, other.keyId), copy2);
        wraps.push({ recipient: `${other.recipient.kind}:${hex(other.keyId)}`, wrap: encode(made) });
      }
    } finally {
      wipe(copy2);
    }
    const place = { company, record: id, revision };
    const body = await sealRevision(await importRecordKey(raw), { ...place, keyId }, encodeRecord(document2));
    const signature = await signRevision(keys.signingPrivate, place, body);
    return { key_id: hex(keyId), body: encode(body), signature: encode(signature), author_key: hex(keys.keyId), wrap: encode(wrap), wraps };
  }
  async function newRecord(company, keys, document2) {
    const id = hex(random(ID_BYTES));
    const { wraps: _none, ...parts } = await sealed(company, keys, id, 1, document2, null, []);
    return { id, body: { id, record_type: document2.type, ...parts } };
  }
  async function revisedRecord(company, keys, vouched, row2, document2, others) {
    return { revision: row2.revision, ...await sealed(company, keys, row2.id, row2.revision + 1, document2, vouched, others) };
  }

  // src/records/share-recipients.ts
  var MOVED = "This record's sharing changed while it was open. Open it again, then make your change.";
  async function sharedWith(held2, id) {
    const path = companyPath(held2);
    const listed = await call("GET", `${path}/records/${encodeURIComponent(id)}/shares`, readShares);
    if (!listed.ok) {
      return { ok: false, words: listed.status === 403 ? "You can read this record but not change it." : loadFailure(listed) };
    }
    const directory = await call("GET", `${path}/keys`, readDirectory);
    if (!directory.ok) {
      return { ok: false, words: loadFailure(directory) };
    }
    const groups = /* @__PURE__ */ new Map();
    for (const share2 of listed.value.shares.filter((each) => each.kind === "group")) {
      const key = await call("GET", `${path}/groups/${encodeURIComponent(share2.target)}/key`, readGroupKey);
      if (!key.ok) {
        return { ok: false, words: key.status === 404 ? MOVED : loadFailure(key) };
      }
      groups.set(share2.target, key.value.public);
    }
    const recipients = revisionRecipients(listed.value, directory.value, groups, hex(held2.keys.keyId));
    return recipients === null ? { ok: false, words: MOVED } : { ok: true, recipients };
  }

  // src/records/ssh-field.ts
  function sshImportProblem(problem2) {
    if (!(problem2 instanceof VaultCryptoError)) {
      return "The page couldn't read this key. Try pasting it again.";
    }
    switch (problem2.reason) {
      case "unknown-algorithm":
        return "Only an Ed25519 key without a passphrase can be kept. Remove the passphrase with ssh-keygen -p, or make a new key here.";
      case "key-mismatch":
        return "The public key in this file doesn't match its private key, so it isn't kept.";
      default:
        return "Paste the whole private key file, from -----BEGIN OPENSSH PRIVATE KEY----- to its END line.";
    }
  }

  // src/records/ssh.ts
  var MAGIC = concat(utf8("openssh-key-v1"), Uint8Array.of(0));
  var KEY_TYPE = "ssh-ed25519";
  var NONE = "none";
  var KEY_BYTES4 = 32;
  var BLOCK = 8;
  var LINE = 70;
  var BEGIN = "-----BEGIN OPENSSH PRIVATE KEY-----";
  var END = "-----END OPENSSH PRIVATE KEY-----";
  function sshString(bytes) {
    return concat(integer(bytes.length, 4), bytes);
  }
  function base64(bytes) {
    return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
  }
  function fromBase64(text3) {
    try {
      return Uint8Array.from(atob(text3), (character) => character.charCodeAt(0));
    } catch {
      throw new VaultCryptoError("malformed");
    }
  }
  function text2(reader) {
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(reader.field());
    } catch {
      throw new VaultCryptoError("malformed");
    }
  }
  async function describe(publicKey, comment, privateKey) {
    const blob = concat(sshString(utf8(KEY_TYPE)), sshString(publicKey));
    const digest2 = new Uint8Array(await crypto.subtle.digest("SHA-256", blob));
    return {
      privateKey,
      publicKey: [KEY_TYPE, base64(blob), comment].filter((part) => part !== "").join(" "),
      fingerprint: `SHA256:${base64(digest2).replace(/=+$/, "")}`
    };
  }
  async function publicOf(seed) {
    const key = await ed25519PrivateFromSeed(seed.slice(), true);
    const jwk = await crypto.subtle.exportKey("jwk", key);
    return fromBase64((jwk.x ?? "").replace(/-/g, "+").replace(/_/g, "/").padEnd(44, "="));
  }
  function privateFile(seed, publicKey, comment) {
    const check = random(4);
    let section = concat(
      check,
      check,
      sshString(utf8(KEY_TYPE)),
      sshString(publicKey),
      sshString(concat(seed, publicKey)),
      sshString(utf8(comment))
    );
    const padding = Uint8Array.from({ length: (BLOCK - section.length % BLOCK) % BLOCK }, (_, index) => index + 1);
    section = concat(section, padding);
    const body = concat(
      MAGIC,
      sshString(utf8(NONE)),
      sshString(utf8(NONE)),
      sshString(new Uint8Array(0)),
      integer(1, 4),
      sshString(concat(sshString(utf8(KEY_TYPE)), sshString(publicKey))),
      sshString(section)
    );
    const encoded = base64(body);
    wipe(section, body);
    const lines = encoded.match(new RegExp(`.{1,${LINE}}`, "g")) ?? [];
    return `${BEGIN}
${lines.join("\n")}
${END}
`;
  }
  async function generateSshKey(comment) {
    const pair = await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
    if (!("privateKey" in pair)) {
      throw new VaultCryptoError("malformed");
    }
    const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
    const seed = pkcs8.slice(pkcs8.length - KEY_BYTES4);
    const publicKey = new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey));
    const key = await describe(publicKey, comment, privateFile(seed, publicKey, comment));
    wipe(pkcs8, seed);
    return key;
  }
  async function importSshKey(pem) {
    const lines = pem.trim().split(/\r?\n/);
    if (lines[0] !== BEGIN || lines.at(-1) !== END) {
      throw new VaultCryptoError("malformed");
    }
    const reader = new Reader(fromBase64(lines.slice(1, -1).join("")));
    if (!sameBytes(reader.take(MAGIC.length), MAGIC)) {
      throw new VaultCryptoError("malformed");
    }
    if (text2(reader) !== NONE || text2(reader) !== NONE) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    if (reader.field().length !== 0 || reader.integer(4) !== 1) {
      throw new VaultCryptoError("malformed");
    }
    const blob = new Reader(reader.field());
    const section = new Reader(reader.field());
    reader.end();
    if (text2(blob) !== KEY_TYPE) {
      throw new VaultCryptoError("unknown-algorithm");
    }
    const publicKey = blob.field();
    blob.end();
    if (section.integer(4) !== section.integer(4) || text2(section) !== KEY_TYPE) {
      throw new VaultCryptoError("malformed");
    }
    const repeated = section.field();
    const pair = section.field();
    const comment = text2(section);
    const padding = section.rest();
    if (publicKey.length !== KEY_BYTES4 || pair.length !== 2 * KEY_BYTES4 || !padding.every((byte, index) => byte === index + 1)) {
      throw new VaultCryptoError("malformed");
    }
    const seed = pair.slice(0, KEY_BYTES4);
    const matches2 = sameBytes(repeated, publicKey) && sameBytes(pair.slice(KEY_BYTES4), publicKey) && sameBytes(await publicOf(seed), publicKey);
    wipe(seed, pair);
    if (!matches2) {
      throw new VaultCryptoError("key-mismatch");
    }
    return describe(publicKey, comment, pem);
  }

  // src/records/ssh-field-view.ts
  function sshKeyControl(label, initial, id) {
    let value = initial;
    const name = el("p", { id: `${id}-label`, class: "field-label" }, label);
    const shown = el("div", { class: "ssh-key" });
    const status = el("p", { class: "muted", role: "status" });
    const hint = el("p", { id: `${id}-hint`, class: "message bad" });
    hint.hidden = true;
    const make = el("button", { type: "button" });
    const paste = el("button", { type: "button", "aria-expanded": "false", "aria-controls": `${id}-paste` });
    const area = el("textarea", { id, rows: "6", spellcheck: "false", autocomplete: "off", "aria-describedby": hint.id });
    const use = el("button", { type: "button", class: "primary" }, "Use this key");
    const pasting = el("div", { id: `${id}-paste`, class: "field" }, el("label", { for: id }, "Private key file"), area, el("div", { class: "input-line" }, use));
    pasting.hidden = true;
    const mark2 = (words) => {
      hint.hidden = words === null;
      hint.textContent = words ?? "";
      if (words === null) {
        area.removeAttribute("aria-invalid");
      } else {
        area.setAttribute("aria-invalid", "true");
      }
    };
    const draw2 = () => {
      fill(shown, ...value === null ? [el("p", { class: "muted" }, "No key yet.")] : [el("p", { class: "value mono" }, value.publicKey), el("p", { class: "muted mono" }, value.fingerprint)]);
      make.textContent = value === null ? "Make a key" : "Make a new key";
      paste.textContent = value === null ? "Paste a key" : "Paste another key";
    };
    const replaced = (next, words) => {
      const had = value !== null;
      value = next;
      draw2();
      status.textContent = had ? `${words} It replaces the old key when you save the record.` : words;
    };
    make.addEventListener("click", () => {
      make.disabled = true;
      status.textContent = "Making a key.";
      void generateSshKey(label).then(
        (key) => {
          make.disabled = false;
          replaced(key, "A new key was made.");
        },
        () => {
          make.disabled = false;
          status.textContent = "";
          mark2("This browser couldn't make an Ed25519 key. Paste one made with ssh-keygen -t ed25519 instead.");
        }
      );
    });
    paste.addEventListener("click", () => {
      const open3 = pasting.hidden;
      pasting.hidden = !open3;
      paste.setAttribute("aria-expanded", String(open3));
      if (open3) {
        area.focus();
      }
    });
    use.addEventListener("click", () => {
      mark2(null);
      void importSshKey(area.value).then(
        (key) => {
          area.value = "";
          pasting.hidden = true;
          paste.setAttribute("aria-expanded", "false");
          replaced(key, "The pasted key is in place.");
          paste.focus();
        },
        (problem2) => mark2(sshImportProblem(problem2))
      );
    });
    draw2();
    const row2 = el("div", { class: "field", role: "group", "aria-labelledby": name.id }, name, shown, el("div", { class: "input-line" }, make, paste), pasting, status, hint);
    return { row: row2, read: () => value, mark: mark2 };
  }

  // src/records/file-transfer.ts
  var PART_BYTES = 4 * 1024 * 1024;
  function partsOf(total) {
    return Array.from({ length: Math.ceil(total / PART_BYTES) }, (_, index) => ({
      offset: index * PART_BYTES,
      length: Math.min(PART_BYTES, total - index * PART_BYTES)
    }));
  }

  // src/records/file-io.ts
  var readCreated = (v) => typeof v === "object" && v !== null && "id" in v && typeof v.id === "string" && /^[0-9a-f]{32}$/.test(v.id) ? v.id : null;
  var filesPath = (held2, record) => `${companyPath(held2)}/records/${encodeURIComponent(record)}/files`;
  async function uploadFile(held2, record, file) {
    if (file.size > FILE_MAX_BYTES) {
      return { ok: false, words: "A file can be up to 256 MiB." };
    }
    const data = new Uint8Array(await file.arrayBuffer());
    const own = newFileKey();
    const created = await call("POST", filesPath(held2, record), readCreated, { body: { size: sealedSize(data.length) } });
    if (!created.ok) {
      return { ok: false, words: loadFailure(created) };
    }
    const context2 = { company: held2.company, record, file: created.value, keyId: fileKeyId(own.keyId) };
    const sealed2 = await sealFile(await importFileKey(own.key), context2, data);
    for (const part of partsOf(sealed2.length)) {
      const sent = await callBytes("PUT", `${filesPath(held2, record)}/${created.value}?offset=${part.offset}`, sealed2.slice(part.offset, part.offset + part.length));
      if (!sent.ok) {
        return { ok: false, words: loadFailure(sent) };
      }
    }
    return { ok: true, value: { id: created.value, name: file.name, size: data.length, mediaType: file.type === "" ? "application/octet-stream" : file.type, ...own } };
  }
  async function downloadFile(held2, record, value) {
    const parts = [];
    for (const part of partsOf(sealedSize(value.size))) {
      const read = await callBytes("GET", `${filesPath(held2, record)}/${encodeURIComponent(value.id)}?offset=${part.offset}&limit=${part.length}`);
      if (!read.ok) {
        return { ok: false, words: loadFailure(read) };
      }
      parts.push(read.value);
    }
    const context2 = { company: held2.company, record, file: value.id, keyId: fileKeyId(value.keyId) };
    try {
      return { ok: true, value: await openFile(/* @__PURE__ */ new Map([[value.keyId, await importFileKey(value.key)]]), context2, concat(...parts)) };
    } catch (problem2) {
      if (problem2 instanceof VaultCryptoError) {
        return { ok: false, words: "This file doesn't check out, so it isn't opened. It may have been changed on the server." };
      }
      throw problem2;
    }
  }

  // src/records/file-field-view.ts
  var sizeWords = (bytes) => new Intl.NumberFormat(void 0, { style: "unit", unit: "kilobyte" }).format(Math.ceil(bytes / 1024));
  var fileWords = (value) => `${value.name} (${sizeWords(value.size)})`;
  function fileControl(label, initial, id, attach) {
    let value = initial;
    const name = el("label", { for: id }, label);
    const shown = el("p", {});
    const status = el("p", { class: "muted", role: "status" });
    const hint = el("p", { id: `${id}-hint`, class: "message bad" });
    hint.hidden = true;
    const mark2 = (words) => {
      hint.hidden = words === null;
      hint.textContent = words ?? "";
    };
    const draw2 = () => {
      fill(shown, value === null ? el("span", { class: "muted" }, "No file yet.") : fileWords(value));
    };
    draw2();
    if (attach === null) {
      const later = el("p", { class: "muted", id }, "Save the record first, then add its file.");
      return { row: el("div", { class: "field" }, el("p", { class: "field-label" }, label), shown, later), read: () => value, mark: mark2 };
    }
    const input = el("input", { id, type: "file", "aria-describedby": hint.id });
    input.addEventListener("change", () => {
      const chosen = input.files?.[0];
      if (chosen === void 0) {
        return;
      }
      mark2(null);
      input.disabled = true;
      status.textContent = `Sealing and sending ${chosen.name}.`;
      void attach(chosen).then((made) => {
        input.disabled = false;
        input.value = "";
        if (!made.ok) {
          status.textContent = "";
          mark2(made.words);
          return;
        }
        const had = value !== null;
        value = made.value;
        draw2();
        status.textContent = had ? `${chosen.name} is sent. It replaces the old file when you save the record.` : `${chosen.name} is sent. Save the record to keep it.`;
      });
    });
    return { row: el("div", { class: "field" }, name, shown, input, status, hint), read: () => value, mark: mark2 };
  }
  async function saveToDevice(held2, record, value) {
    announce(`Opening ${value.name}.`);
    const opened = await downloadFile(held2, record, value);
    if (!opened.ok) {
      announce(opened.words);
      return;
    }
    const url = URL.createObjectURL(new Blob([opened.value], { type: value.mediaType }));
    const link = el("a", { href: url, download: value.name });
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 0);
    announce(`${value.name} is ready in your downloads.`);
  }

  // src/records/edit-view.ts
  var INPUT = {
    text: "text",
    concealed: "password",
    url: "url",
    email: "email",
    phone: "tel",
    number: "text",
    date: "date",
    "month-year": "month"
  };
  function control(field3, id, attach) {
    if (field3.type === "file") {
      return fileControl(field3.label, field3.value, id, attach);
    }
    if (field3.type === "ssh-key") {
      return sshKeyControl(field3.label, field3.value, id);
    }
    const hint = el("p", { id: `${id}-hint`, class: "message bad" });
    hint.hidden = true;
    const label = el("label", { for: id }, field3.label);
    let input = null;
    let read = () => field3.value;
    if (field3.type === "boolean") {
      const box2 = el("input", { id, type: "checkbox" });
      box2.checked = field3.value === true;
      read = () => box2.checked;
      input = box2;
    } else if (field3.type === "multiline") {
      const area = el("textarea", { id, rows: "4" });
      area.value = field3.value ?? "";
      read = () => area.value;
      input = area;
    } else if (INPUT[field3.type] !== void 0 && (typeof field3.value === "string" || field3.value === null)) {
      const text3 = el("input", { id, type: INPUT[field3.type] ?? "text", autocomplete: field3.type === "concealed" ? "new-password" : "off" });
      text3.value = typeof field3.value === "string" ? field3.value : "";
      read = () => text3.value;
      input = text3;
    }
    input?.setAttribute("aria-describedby", hint.id);
    const kept = input === null ? el("p", { class: "muted" }, "Kept as it is. Editing this kind of field comes later.") : null;
    const mark2 = (words) => {
      hint.hidden = words === null;
      hint.textContent = words ?? "";
      if (words === null) {
        input?.removeAttribute("aria-invalid");
      } else {
        input?.setAttribute("aria-invalid", "true");
      }
    };
    return { row: el("div", { class: "field" }, label, input, kept, hint), read, mark: mark2 };
  }
  function saveFailure(code, fallback) {
    switch (code) {
      case "stale_revision":
        return "This record changed since you opened it. Open it again to see the newer version, then make your change.";
      case "stale_shares":
        return "This record's sharing changed while it was open. Open it again, then make your change.";
      default:
        return fallback;
    }
  }
  function form(main2, title, document2, trail, extra, attach, save) {
    const name = el("input", { id: "record-title", type: "text", autocomplete: "off", value: document2.title });
    const nameHint = el("p", { id: "record-title-hint", class: "message bad" });
    nameHint.hidden = true;
    name.setAttribute("aria-describedby", nameHint.id);
    const controls = /* @__PURE__ */ new Map();
    const { base, custom } = splitCustom(document2);
    const own = customEditor(custom, (field3, id) => control(field3, id, attach), base.sections.flatMap((section) => section.fields.map((field3) => field3.label)));
    const sections = base.sections.map(
      (section, sectionIndex) => el(
        "fieldset",
        { class: "record-section" },
        section.name === "" ? null : el("legend", {}, section.name),
        ...section.fields.map((field3, fieldIndex) => {
          const at2 = fieldAt(sectionIndex, fieldIndex);
          const made = control(field3, `field-${sectionIndex}-${fieldIndex}`, attach);
          controls.set(at2, made);
          return made.row;
        })
      )
    );
    const submit = el("button", { type: "submit", class: "primary" }, "Save record");
    const said = el("div", { "aria-live": "polite" });
    const body = el("form", { novalidate: "" }, el("div", { class: "field" }, el("label", { for: name.id }, "Title"), name, nameHint), ...sections, own.section, extra, submit, said);
    body.addEventListener("submit", (event) => {
      event.preventDefault();
      const added = own.current();
      const all = new Map(controls);
      added.forEach((entry, index) => all.set(fieldAt(base.sections.length, index), entry.control));
      const values = new Map([...all].map(([at2, made]) => [at2, made.read()]));
      const result = edited(joinCustom(base, added.map((entry) => entry.field)), name.value, values);
      nameHint.hidden = result.ok || !result.problems.has("title");
      nameHint.textContent = result.ok ? "" : result.problems.get("title") ?? "";
      for (const [at2, made] of all) {
        made.mark(result.ok ? null : result.problems.get(at2) ?? null);
      }
      if (!result.ok) {
        fill(said, refusal("Some values can't be kept. Each one says why below its label."));
        body.querySelector("[aria-invalid=true]")?.focus();
        if (result.problems.has("title")) {
          name.setAttribute("aria-invalid", "true");
          name.focus();
        }
        return;
      }
      name.removeAttribute("aria-invalid");
      submit.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Sealing and saving the record."));
      void save(result.document).then((refused) => {
        submit.disabled = false;
        fill(said, refused === null ? null : refusal(refused));
      });
    });
    const head = el("div", { class: "page-head" }, el("div", {}, heading(title), el("p", { class: "lede" }, typeName(document2.type))));
    fill(main2, breadcrumbs(trail), head, body);
    name.focus();
  }
  async function drawNew(main2, held2, type, back, chooser, opened, newOf) {
    const template = BUILT_IN_TYPES.find((each) => each.id === type);
    if (template === void 0) {
      const choices = el("ul", { class: "records", "aria-label": "Record types" }, ...BUILT_IN_TYPES.map((each) => el("li", { class: "record" }, el("a", { class: "record-title", href: newOf(each.id) }, each.name))));
      fill(main2, breadcrumbs(newCrumbs(back, chooser, null)), el("div", { class: "page-head" }, heading("New record")), el("p", {}, "Choose what kind of record to make."), choices);
      return;
    }
    const trail = newCrumbs(back, chooser, template.name);
    fill(main2, breadcrumbs(trail), el("div", { class: "page-head" }, heading("New record")), el("p", { class: "muted", role: "status" }, "Opening the form."));
    const audience = await loadAudience(held2);
    if (!audience.ok) {
      fill(main2, breadcrumbs(trail), el("div", { class: "page-head" }, heading("New record")), refusal(audience.words));
      return;
    }
    const picker = audiencePicker(audience.choices);
    form(main2, "New record", recordFromTemplate(template, ""), trail, picker.section, null, async (made) => {
      const record = await newRecord(held2.company, held2.keys, made);
      const answer = await call("POST", `${companyPath(held2)}/records`, readRow, { body: record.body });
      if (!answer.ok) {
        return saveFailure(answer.code, loadFailure(answer));
      }
      const to = picker.chosen()?.recipient ?? null;
      const unshared = to === null ? null : await shareCreated(held2, record.id, to);
      opened(record.id);
      if (unshared !== null) {
        announce(`The record was saved for you only. ${unshared}`);
      }
      return null;
    });
  }
  async function drawEdit(main2, held2, id, records, back, opened) {
    const unnamed = breadcrumbs(editCrumbs(records, back, null));
    fill(main2, unnamed, el("div", { class: "page-head" }, heading("Edit record")), el("p", { class: "muted", role: "status" }, "Opening the record."));
    const [chain, answer] = await Promise.all([vouchedChain(held2), call("GET", `${companyPath(held2)}/records/${encodeURIComponent(id)}`, readOpened)]);
    if (!chain.ok) {
      fill(main2, unnamed, el("div", { class: "page-head" }, heading("Edit record")), refusal("problem" in chain ? CHAIN_REFUSED : loadFailure(chain)));
      return;
    }
    if (!answer.ok) {
      fill(main2, unnamed, el("div", { class: "page-head" }, heading("Edit record")), refusal(loadFailure(answer)));
      return;
    }
    const item = await new Opener(held2, chain.vouched).open(answer.value);
    if (!("document" in item)) {
      fill(main2, unnamed, el("div", { class: "page-head" }, heading("Record not shown")), refusal(unreadableWords(item.problem)));
      return;
    }
    const row2 = item.row;
    form(main2, "Edit record", item.document, editCrumbs(records, back, item.document.title), null, (file) => uploadFile(held2, id, file), async (made) => {
      const sharing = await sharedWith(held2, id);
      if (!sharing.ok) {
        return sharing.words;
      }
      const revision = await revisedRecord(held2.company, held2.keys, chain.vouched, row2, made, sharing.recipients);
      const saved = await call("PUT", `${companyPath(held2)}/records/${encodeURIComponent(id)}`, readRow, { body: revision });
      if (!saved.ok) {
        return saveFailure(saved.code, loadFailure(saved));
      }
      opened(id);
      return null;
    });
  }

  // src/records/field-view.ts
  var MASK = "••••••••••••";
  function copyButton(copy2, label, value) {
    const button = iconAction("copy", "Copy", `Copy ${label}`);
    button.addEventListener("click", () => void copy2.copy(value(), label));
    return button;
  }
  function concealed(label, value, copy2) {
    const shown = el("span", { class: "value mono" }, MASK);
    const toggle = iconAction("eye", "Show", `Show ${label}`);
    toggle.setAttribute("aria-pressed", "false");
    toggle.addEventListener("click", () => {
      const showing = toggle.getAttribute("aria-pressed") !== "true";
      toggle.setAttribute("aria-pressed", String(showing));
      toggle.setAttribute("aria-label", `${showing ? "Hide" : "Show"} ${label}`);
      toggle.dataset["tip"] = showing ? "Hide" : "Show";
      toggle.replaceChildren(icon(showing ? "eye-off" : "eye"));
      shown.textContent = showing ? value : MASK;
    });
    return el("span", { class: "value-row" }, shown, el("span", { class: "row-actions" }, toggle, copyButton(copy2, label, () => value)));
  }
  function plain(label, value, copy2, link = false) {
    const safeLink = link && /^https?:\/\//i.test(value);
    const shown = safeLink ? el("a", { class: "value", href: value, target: "_blank", rel: "noopener noreferrer" }, value) : el("span", { class: "value" }, value);
    return el("span", { class: "value-row" }, shown, el("span", { class: "row-actions" }, copyButton(copy2, label, () => value)));
  }
  function oneTime(label, value, copy2, stops2) {
    const code = el("span", { class: "value mono" }, "");
    const left = el("span", { class: "muted" }, "");
    let current = "";
    const params = { secret: base32Decode(value.secret), algorithm: value.algorithm, digits: value.digits, period: value.period };
    const tick = async () => {
      const now = Math.floor(Date.now() / 1e3);
      current = await totpCode(params, now);
      code.textContent = `${current.slice(0, current.length / 2)} ${current.slice(current.length / 2)}`;
      left.textContent = `${value.period - now % value.period} s left`;
    };
    void tick();
    const timer = window.setInterval(() => void tick(), 1e3);
    stops2.push(() => window.clearInterval(timer));
    return el("span", { class: "value-row" }, code, left, el("span", { class: "row-actions" }, copyButton(copy2, label, () => current)));
  }
  function fieldView(field3, copy2, stops2, save) {
    const term = el("dt", {}, field3.label);
    if (field3.value === null) {
      return [term, el("dd", { class: "muted" }, "Empty")];
    }
    switch (field3.type) {
      case "concealed":
        return [term, el("dd", {}, concealed(field3.label, field3.value, copy2))];
      case "url":
        return [term, el("dd", {}, plain(field3.label, field3.value, copy2, true))];
      case "multiline":
        return [term, el("dd", {}, el("span", { class: "value-row" }, el("span", { class: "value pre" }, field3.value), el("span", { class: "row-actions" }, copyButton(copy2, field3.label, () => field3.value ?? ""))))];
      case "boolean":
        return [term, el("dd", {}, field3.value ? "Yes" : "No")];
      case "otp":
        return [term, el("dd", {}, oneTime(field3.label, field3.value, copy2, stops2))];
      case "file": {
        const value = field3.value;
        const download = iconAction("download", "Download", `Download ${value.name}`);
        download.addEventListener("click", () => save(value));
        return [term, el("dd", {}, el("span", { class: "value-row" }, el("span", { class: "value" }, fileWords(value)), el("span", { class: "row-actions" }, download)))];
      }
      case "security-question":
        return [term, el("dd", {}, el("p", {}, field3.value.question), concealed(`${field3.label} answer`, field3.value.answer, copy2))];
      case "ssh-key":
        return [
          term,
          el(
            "dd",
            {},
            plain(`${field3.label} public key`, field3.value.publicKey, copy2),
            el("p", { class: "muted mono" }, field3.value.fingerprint),
            concealed(`${field3.label} private key`, field3.value.privateKey, copy2)
          )
        ];
      default:
        return [term, el("dd", {}, plain(field3.label, field3.value, copy2))];
    }
  }

  // src/records/delete-words.ts
  function deleteFailure(status, code, fallback) {
    if (status === 403) {
      return "Only the record's author or a company admin can delete it.";
    }
    if (status === 404) {
      return "This record is already gone, or it isn't shared with you.";
    }
    if (status === 409 && code === "stale_revision") {
      return "This record changed while you were deciding. Open it again, then delete it.";
    }
    return fallback;
  }

  // src/records/delete-dialog.ts
  function deleteControls(held2, id, title, gone) {
    const reason = el("input", { id: "delete-reason", type: "text", autocomplete: "off", maxlength: "500" });
    const said = el("div", { "aria-live": "polite" });
    const keep2 = el("button", { type: "button", autofocus: "" }, "Keep the record");
    const confirm = el("button", { type: "button", class: "danger" }, "Delete record");
    const dialog = el(
      "dialog",
      { class: "confirm", "aria-labelledby": "delete-title" },
      el("h2", { id: "delete-title" }, `Delete “${title}”?`),
      el(
        "p",
        {},
        "The record, its files and its history are removed for everyone it is shared with. Backups taken before now keep a sealed copy until they expire."
      ),
      el("div", { class: "field" }, el("label", { for: reason.id }, "Reason, kept on the company's trail (optional)"), reason),
      said,
      el("div", { class: "actions" }, keep2, confirm)
    );
    const open3 = iconAction("trash", "Delete", `Delete ${title}`, "danger");
    open3.addEventListener("click", () => {
      fill(said, null);
      reason.value = "";
      dialog.showModal();
    });
    keep2.addEventListener("click", () => dialog.close());
    confirm.addEventListener("click", () => {
      confirm.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Deleting the record."));
      void call("DELETE", `${companyPath(held2)}/records/${encodeURIComponent(id)}`, ignored, { reason: reason.value.trim() }).then((answer) => {
        confirm.disabled = false;
        if (answer.ok) {
          dialog.close();
          gone();
          return;
        }
        fill(said, refusal(deleteFailure(answer.status, answer.code, loadFailure(answer))));
      });
    });
    return [open3, dialog];
  }

  // src/records/share-dialog.ts
  var RIGHTS = [
    { right: "read", words: "Can view" },
    { right: "edit", words: "Can edit" }
  ];
  function shareControls(context2, targets) {
    let chosen = null;
    let right = "read";
    const filter = el("input", { id: "share-filter", type: "search", autocomplete: "off", "aria-controls": "share-targets" });
    const list = el("div", { id: "share-targets", class: "targets" });
    const said = el("div", { "aria-live": "polite" });
    const rights = RIGHTS.map((each) => el("button", { type: "button", class: "chip", "aria-pressed": String(each.right === right) }, each.words));
    const cancel = el("button", { type: "button" }, "Cancel");
    const confirm = el("button", { type: "button", class: "primary" }, "Share");
    const draw2 = () => {
      const shown = filterTargets(targets, filter.value);
      if (shown.length === 0) {
        fill(list, el("p", { class: "muted" }, targets.length === 0 ? "Everyone in the company can open this record already." : `No member or group matches “${filter.value.trim()}”.`));
        return;
      }
      fill(list, ...shown.map((target) => targetRow(target, target.id === chosen, () => chosen = target.id)));
    };
    const press = (at2) => {
      right = RIGHTS[at2]?.right ?? "read";
      rights.forEach((each, other) => each.setAttribute("aria-pressed", String(other === at2)));
    };
    rights.forEach((button, at2) => button.addEventListener("click", () => press(at2)));
    filter.addEventListener("input", draw2);
    const dialog = el(
      "dialog",
      { class: "confirm", "aria-labelledby": "share-title" },
      el("h2", { id: "share-title" }, `Share “${context2.title}”`),
      el("div", { class: "field" }, el("label", { for: filter.id }, "Find a member or group"), filter),
      el("fieldset", { class: "choices" }, el("legend", {}, "Share with"), list),
      el("fieldset", { class: "choices" }, el("legend", {}, "What they can do"), el("div", { class: "chips" }, ...rights)),
      said,
      el("div", { class: "actions" }, cancel, confirm)
    );
    const open3 = iconAction("share", "Share", `Share ${context2.title}`);
    open3.addEventListener("click", () => {
      chosen = null;
      press(0);
      filter.value = "";
      fill(said, null);
      draw2();
      dialog.showModal();
    });
    cancel.addEventListener("click", () => dialog.close());
    confirm.addEventListener("click", () => {
      const target = targets.find((each) => each.id === chosen);
      if (target?.recipient == null) {
        fill(said, refusal("Choose a member or group to share with."));
        return;
      }
      confirm.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Sharing the record."));
      void share(context2, target.recipient, right).then((words) => {
        confirm.disabled = false;
        if (words === null) {
          dialog.close();
          context2.done();
          return;
        }
        fill(said, refusal(words));
      });
    });
    return [open3, dialog];
  }
  function targetRow(target, checked, choose) {
    const id = `share-target-${target.id}`;
    const radio = el("input", { id, type: "radio", name: "share-target", value: target.id, ...checked ? { checked: "" } : {} });
    if (target.recipient === null) {
      radio.disabled = true;
    }
    radio.addEventListener("change", choose);
    const detail = target.recipient === null ? `${target.detail} · ${target.missing}` : target.detail;
    return el("div", { class: "choice" }, radio, el("label", { for: id }, target.label, el("span", { class: "detail" }, detail)));
  }
  async function share(context2, to, right) {
    const { held: held2, vouched, opener, opened } = context2;
    let body;
    try {
      body = await shareBody(held2.company, vouched, await opener.keyFor(opened), opened, to, right);
    } catch (problem2) {
      return sealFailure(problem2 instanceof VaultCryptoError ? problem2.reason : "malformed");
    }
    const answer = await call("POST", `${companyPath(held2)}/records/${encodeURIComponent(opened.row.id)}/shares`, ignored, { body });
    return answer.ok ? null : shareFailure(answer.status, answer.code, loadFailure(answer));
  }

  // src/records/shared-with-view.ts
  function sharedWithSection(held2, id, rows, done) {
    const reason = el("input", { id: "unshare-reason", type: "text", autocomplete: "off", maxlength: "500" });
    const said = el("div", { "aria-live": "polite" });
    const title = el("h2", { id: "unshare-title" });
    const keep2 = el("button", { type: "button", autofocus: "" }, "Keep sharing");
    const confirm = el("button", { type: "button", class: "danger" }, "Stop sharing");
    const dialog = el(
      "dialog",
      { class: "confirm", "aria-labelledby": title.id },
      title,
      el(
        "p",
        {},
        "They can no longer open this record. What they have already seen, and backups taken before now, are not taken back. The record's next change is sealed under a new key."
      ),
      el("div", { class: "field" }, el("label", { for: reason.id }, "Reason, kept on the company's trail (optional)"), reason),
      said,
      el("div", { class: "actions" }, keep2, confirm)
    );
    let removing = null;
    keep2.addEventListener("click", () => dialog.close());
    confirm.addEventListener("click", () => {
      if (removing === null) {
        return;
      }
      confirm.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Removing the share."));
      const route = `${companyPath(held2)}/records/${encodeURIComponent(id)}/shares/${encodeURIComponent(removing.target)}`;
      void call("DELETE", route, ignored, { reason: reason.value.trim() }).then((answer) => {
        confirm.disabled = false;
        if (answer.ok) {
          dialog.close();
          done();
          return;
        }
        fill(said, refusal(removeShareFailure(answer.status, answer.code, loadFailure(answer))));
      });
    });
    const items = rows.map((row2) => {
      const remove = iconAction("close", "Stop sharing", `Stop sharing with ${row2.label}`, "danger");
      remove.addEventListener("click", () => {
        removing = row2;
        fill(title, `Stop sharing with “${row2.label}”?`);
        fill(said, null);
        reason.value = "";
        dialog.showModal();
      });
      return el(
        "li",
        { class: "share" },
        el("div", {}, el("span", { class: "share-name" }, row2.label), row2.detail === "" ? null : el("span", { class: "detail" }, row2.detail)),
        el("span", { class: "muted" }, row2.right === "edit" ? "Can edit" : "Can view"),
        remove
      );
    });
    const section = el(
      "section",
      { class: "record-section", "aria-labelledby": "shared-with" },
      el("h2", { id: "shared-with" }, "Shared with"),
      items.length === 0 ? el("p", { class: "muted" }, "It isn't shared with anyone; only its author can open it.") : el("ul", { class: "shares" }, ...items)
    );
    return [section, dialog];
  }

  // src/records/record-view.ts
  async function drawRecord(main2, held2, id, copy2, stops2, back, editHref, gone) {
    const unnamed = breadcrumbs(recordCrumbs(back, null));
    const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Opening the record."));
    fill(main2, unnamed, el("div", { class: "page-head" }, heading("Record")), said);
    const [chain, opened] = await Promise.all([
      vouchedChain(held2),
      call("GET", `${companyPath(held2)}/records/${encodeURIComponent(id)}`, readOpened)
    ]);
    if (!chain.ok) {
      fill(said, refusal("problem" in chain ? CHAIN_REFUSED : loadFailure(chain)));
      return;
    }
    if (!opened.ok) {
      fill(said, refusal(opened.status === 404 ? "This record doesn't exist, or it isn't shared with you." : loadFailure(opened)));
      return;
    }
    const opener = new Opener(held2, chain.vouched);
    const [item, sharing] = await Promise.all([opener.open(opened.value), loadSharing(held2, id)]);
    if (!("document" in item)) {
      fill(main2, unnamed, el("div", { class: "page-head" }, heading("Record not shown")), refusal(unreadableWords(item.problem)));
      return;
    }
    const { document: document2 } = item;
    const title = heading(document2.title === "" ? "Untitled" : document2.title);
    const sections = document2.sections.map(
      (section) => el(
        "section",
        { class: "record-section" },
        section.name === "" ? null : el("h2", {}, section.name),
        el("dl", { class: "fields" }, ...section.fields.flatMap((field3) => fieldView(field3, copy2, stops2, (value) => void saveToDevice(held2, id, value))))
      )
    );
    const head = el("div", {}, title, el("p", { class: "lede" }, typeName(document2.type)));
    const named2 = document2.title === "" ? "Untitled" : document2.title;
    const [remove, dialog] = deleteControls(held2, id, named2, gone);
    const again = () => void drawRecord(main2, held2, id, copy2, stops2, back, editHref, gone);
    const context2 = { held: held2, vouched: chain.vouched, opener, opened: opened.value, title: named2, done: again };
    const [share2, shareDialog] = sharing?.ok === true ? shareControls(context2, sharing.targets) : [];
    const shared = sharing === null ? [] : sharing.ok ? sharedWithSection(held2, id, sharing.rows, again) : [el("section", { class: "record-section" }, el("h2", {}, "Shared with"), refusal(sharing.words))];
    const edit = el("a", { class: "button icon-action quiet", href: editHref, "aria-label": `Edit ${named2}`, "data-tip": "Edit" }, icon("edit"));
    const actions = el("div", { class: "actions" }, edit, share2 ?? null, remove ?? null);
    fill(main2, breadcrumbs(recordCrumbs(back, document2.title)), el("div", { class: "page-head" }, head, actions), ...sections, ...shared, dialog ?? null, shareDialog ?? null);
    title.focus();
  }

  // src/crypto/fingerprint.ts
  var SHOWN_BYTES = 16;
  async function keyFingerprint(agreementPublic, signingPublic) {
    const digest2 = new Uint8Array(await crypto.subtle.digest("SHA-256", concat(utf8(LABEL.fingerprint), field2(agreementPublic), field2(signingPublic))));
    const digits = [...digest2.subarray(0, SHOWN_BYTES)].map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join("");
    return digits.match(/.{4}/g)?.join(" ") ?? digits;
  }

  // src/members/members-model.ts
  async function memberRows(members, directory, vouched) {
    return Promise.all(
      members.filter((member) => member.active).map(async (member) => {
        const listed = directory.get(member.account);
        const base = { account: member.account, label: member.name, detail: member.email, role: member.role };
        if (listed === void 0) {
          return { ...base, state: "no-key", fingerprint: null };
        }
        const agreement = vouched.agreementKeyOf(member.account);
        const signing = vouched.signingKeyOf(member.account);
        const matches2 = agreement !== void 0 && signing !== void 0 && sameBytes(agreement, listed.agreementPublic) && sameBytes(signing, listed.signingPublic);
        return { ...base, state: matches2 ? "vouched" : "waiting", fingerprint: await keyFingerprint(listed.agreementPublic, listed.signingPublic) };
      })
    );
  }
  async function vouchStatement(company, ownerSigning, chain, signer, account, key) {
    const statement = await appendStatement(company, chain.at(-1), { kind: "member", subject: account, agreementPublic: key.agreementPublic, signingPublic: key.signingPublic }, signer);
    await verifyChain(company, ownerSigning, [...chain, statement]);
    return statement;
  }
  var statementBody = (seq, statement) => ({
    seq,
    bytes: encode(statement.bytes),
    signature: encode(statement.signature)
  });

  // src/members/members-words.ts
  function keyStateWords(state) {
    switch (state) {
      case "vouched":
        return "Key vouched for";
      case "waiting":
        return "Key waiting to be vouched for";
      case "no-key":
        return "Hasn't set an unlock password yet";
    }
  }
  function vouchFailure(status, code, fallback) {
    if (status === 403) {
      return "Only the company's owner or an admin can vouch for keys.";
    }
    if (status === 409 && code === "stale_chain") {
      return "Someone vouched for a key meanwhile. The list is shown again; vouch once more.";
    }
    return fallback;
  }
  function vouchRefused(reason) {
    return reason === "unvouched-signer" ? "Only the company's owner or an admin the owner vouched for can vouch for keys, and your key isn't one of them yet." : "Your company's list of who may read what doesn't check out, so nothing was vouched for. Tell your company's owner.";
  }

  // src/members/members-view.ts
  var ROLE_WORDS = { owner: "Owner", admin: "Admin", member: "Member" };
  async function drawMembers(main2, held2, notice2 = null) {
    const title = heading("Members");
    const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Loading the members."));
    fill(main2, el("div", { class: "page-head" }, title), said);
    const path = companyPath(held2);
    const [members, directory, chain] = await Promise.all([
      call("GET", `${path}/members`, readMemberList),
      call("GET", `${path}/keys`, readDirectory),
      call("GET", `${path}/statements`, readChain)
    ]);
    if (!members.ok || !directory.ok || !chain.ok) {
      const failed2 = !members.ok ? members : !directory.ok ? directory : chain;
      if (!failed2.ok) {
        fill(said, refusal(loadFailure(failed2)));
      }
      return;
    }
    const owner = held2.keys.ownerSigningPublic;
    const vouched = await verifyChain(held2.company, owner, chain.value.statements).catch(() => null);
    if (vouched === null) {
      fill(said, refusal(CHAIN_REFUSED));
      return;
    }
    const rows = await memberRows(members.value, directory.value, vouched);
    const me = rows.find((row2) => row2.account === held2.account);
    const mayVouch = me?.role === "owner" || me?.role === "admin";
    const again = (words = null) => void drawMembers(main2, held2, words);
    const fingerprint = el("p", { class: "fingerprint" });
    const dialogSaid = el("div", { "aria-live": "polite" });
    const dialogTitle = el("h2", { id: "vouch-title" });
    const notNow = el("button", { type: "button", autofocus: "" }, "Not now");
    const confirm = el("button", { type: "button", class: "primary" }, "Vouch for this key");
    const dialog = el(
      "dialog",
      { class: "confirm", "aria-labelledby": dialogTitle.id },
      dialogTitle,
      el("p", {}, "Ask them to read out the fingerprint on their own Members page, by phone or in person. Vouch only if every group matches."),
      fingerprint,
      dialogSaid,
      el("div", { class: "actions" }, notNow, confirm)
    );
    let chosen = null;
    notNow.addEventListener("click", () => dialog.close());
    confirm.addEventListener("click", () => {
      const row2 = chosen;
      const key = row2 === null ? void 0 : directory.value.get(row2.account);
      if (row2 === null || key === void 0) {
        return;
      }
      confirm.disabled = true;
      fill(dialogSaid, el("p", { class: "muted", role: "status" }, "Vouching for the key."));
      const signer = { privateKey: held2.keys.signingPrivate, publicBytes: held2.keys.signingPublic };
      void vouchStatement(held2.company, owner, chain.value.statements, signer, row2.account, key).then(async (statement) => {
        const answer = await call("POST", `${path}/statements`, ignored, { body: statementBody(chain.value.statements.length + 1, statement) });
        confirm.disabled = false;
        if (answer.ok) {
          dialog.close();
          again();
          return;
        }
        const words = vouchFailure(answer.status, answer.code, loadFailure(answer));
        if (answer.code === "stale_chain") {
          dialog.close();
          again(words);
          return;
        }
        fill(dialogSaid, refusal(words));
      }).catch((problem2) => {
        confirm.disabled = false;
        fill(dialogSaid, refusal(vouchRefused(problem2 instanceof VaultCryptoError ? problem2.reason : "chain-broken")));
      });
    });
    const items = rows.map((row2) => {
      const vouch = mayVouch && row2.state === "waiting" ? el("button", { type: "button", "aria-label": `Vouch for ${row2.label}'s key` }, "Vouch") : null;
      vouch?.addEventListener("click", () => {
        chosen = row2;
        fill(dialogTitle, `Vouch for “${row2.label}”'s key?`);
        fill(fingerprint, row2.fingerprint ?? "");
        fill(dialogSaid, null);
        dialog.showModal();
      });
      return el(
        "li",
        { class: "share member" },
        el("div", {}, el("span", { class: "share-name" }, row2.account === held2.account ? `${row2.label} (you)` : row2.label), el("span", { class: "detail" }, row2.detail)),
        el("span", { class: "muted" }, ROLE_WORDS[row2.role]),
        el("span", { class: `state ${row2.state}` }, keyStateWords(row2.state)),
        vouch ?? el("span", {})
      );
    });
    const own = me?.fingerprint ?? null;
    fill(
      main2,
      el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "Records are shared only with keys an owner or admin has vouched for, after comparing the key's fingerprint with its holder."))),
      notice2 === null ? null : refusal(notice2),
      own === null ? null : el("section", { class: "record-section", "aria-labelledby": "own-key" }, el("h2", { id: "own-key" }, "Your key's fingerprint"), el("p", { class: "fingerprint" }, own)),
      el("section", { class: "record-section", "aria-labelledby": "everyone" }, el("h2", { id: "everyone" }, "Everyone in the company"), el("ul", { class: "shares" }, ...items)),
      dialog
    );
    title.focus();
  }

  // src/groups/group-members.ts
  var isRecord7 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isId4 = (v) => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
  var isText5 = (v) => typeof v === "string" && v !== "";
  function readGroupMembers(v) {
    if (!isRecord7(v) || !Array.isArray(v["members"])) {
      return null;
    }
    const members = v["members"].map(
      (member) => isRecord7(member) && isId4(member["id"]) && isText5(member["email"]) && isText5(member["name"]) ? { account: member["id"], email: member["email"], name: member["name"] } : null
    );
    return members.every((member) => member !== null) ? members : null;
  }
  function addable(company, inGroup) {
    const inside = new Set(inGroup.map((member) => member.account));
    return company.filter((member) => member.active && !inside.has(member.account)).sort((one, other) => one.name.localeCompare(other.name));
  }

  // src/groups/group-words.ts
  function keyButton(group, rotate) {
    return rotate ? { text: "Make a new key", label: `Make a new key for ${group}` } : { text: "Key this group", label: `Key this group, ${group}` };
  }
  function giveButton(group, waiting) {
    return { text: `Give access (${waiting} waiting)`, label: `Give access (${waiting} waiting), ${group}` };
  }
  function takeOutButton(person, group) {
    return { text: "Take out", label: `Take out ${person} from ${group}` };
  }
  var groupCrumbs = (groups, name) => [
    { label: "Groups", href: groups },
    { label: name ?? "Group", href: null }
  ];

  // src/groups/groups-model.ts
  var KEY_BYTES5 = 32;
  var isRecord8 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  var isId5 = (v) => typeof v === "string" && /^[0-9a-f]{32}$/.test(v);
  var isText6 = (v) => typeof v === "string" && v !== "";
  function readGroupRows(v) {
    if (!isRecord8(v) || !Array.isArray(v["groups"])) {
      return null;
    }
    const rows = v["groups"].map((group) => {
      if (!isRecord8(group) || !isId5(group["id"]) || !isText6(group["name"]) || !isId5(group["parent"])) {
        return null;
      }
      const { kind } = group;
      return kind === "department" || kind === "team" ? { id: group["id"], kind, name: group["name"], parent: group["parent"] } : null;
    });
    return rows.every((row2) => row2 !== null) ? rows : null;
  }
  function groupTree(company, rows) {
    const byName = (one, other) => one.name.localeCompare(other.name);
    const teamsOf = (parent, depth) => rows.filter((row2) => row2.kind === "team" && row2.parent === parent).sort(byName).map((row2) => ({ id: row2.id, kind: row2.kind, name: row2.name, depth }));
    const departments = rows.filter((row2) => row2.kind === "department").sort(byName).flatMap((row2) => [{ id: row2.id, kind: row2.kind, name: row2.name, depth: 1 }, ...teamsOf(row2.id, 2)]);
    return [{ id: company.id, kind: "company", name: company.name, depth: 0 }, ...departments, ...teamsOf(company.id, 1)];
  }
  function readPending(v) {
    if (!isRecord8(v) || !Array.isArray(v["pending"])) {
      return null;
    }
    const keys = v["pending"].map((item) => {
      if (!isRecord8(item) || !isId5(item["account"]) || typeof item["key_id"] !== "string" || typeof item["agreement_public"] !== "string") {
        return null;
      }
      const keyId = keyIdBytes(item["key_id"]);
      const agreementPublic = decode(item["agreement_public"]);
      return keyId === null || agreementPublic === null || agreementPublic.length !== KEY_BYTES5 ? null : { account: item["account"], keyId, agreementPublic };
    });
    return keys.every((key) => key !== null) ? keys : null;
  }
  function createFailure(code, fallback) {
    switch (code) {
      case "wrong_parent":
        return "A department sits in the company; a team sits in a department or in the company.";
      case "forbidden":
        return "Only an owner or an admin can make departments and teams.";
      case "group_not_empty":
        return "This department still holds teams. Delete or move them first.";
      default:
        return fallback;
    }
  }
  function readRotationDue(v) {
    return isRecord8(v) && typeof v["rotation_due"] === "boolean" ? v["rotation_due"] : null;
  }

  // src/groups/group-page.ts
  var KIND_WORDS = { department: "Department", team: "Team" };
  var labelled = (words) => el("button", { type: "button", "aria-label": words.label }, words.text);
  async function drawGroup(main2, held2, id, groups, gone, notice2 = null) {
    const base = `${companyPath(held2)}/groups/${encodeURIComponent(id)}`;
    fill(main2, breadcrumbs(groupCrumbs(groups, null)), el("div", { class: "page-head" }, heading("Group")), el("p", { class: "muted", role: "status" }, "Loading the group."));
    const [listed, inGroup, company] = await Promise.all([
      call("GET", `${companyPath(held2)}/groups`, readGroupRows),
      call("GET", `${base}/members`, readGroupMembers),
      call("GET", `${companyPath(held2)}/members`, readMemberList)
    ]);
    const failed2 = [listed, inGroup, company].find((answer) => !answer.ok);
    const row2 = listed.ok ? listed.value.find((each) => each.id === id) : void 0;
    if (failed2 !== void 0 || !listed.ok || !inGroup.ok || !company.ok || row2 === void 0) {
      const words = failed2 !== void 0 && !failed2.ok ? loadFailure(failed2) : "This group doesn't exist any more.";
      fill(main2, breadcrumbs(groupCrumbs(groups, null)), el("div", { class: "page-head" }, heading("Group")), refusal(words));
      return;
    }
    const said = el("div", { "aria-live": "polite" }, notice2 === null ? null : el("p", { class: "muted", role: "status" }, notice2));
    const again = (words = null) => void drawGroup(main2, held2, id, groups, gone, words);
    const send = (method, url, payload, then) => {
      fill(said, el("p", { class: "muted", role: "status" }, "Saving."));
      void call(method, url, ignored, payload === void 0 ? {} : { body: payload }).then((answer) => {
        if (!answer.ok) {
          fill(said, refusal(answer.code === "not_a_member" ? "Only an active member of the company can join a group." : createFailure(answer.code, loadFailure(answer))));
          return;
        }
        then();
      });
    };
    const people = inGroup.value.map((member) => {
      const remove2 = labelled(takeOutButton(member.name, row2.name));
      remove2.addEventListener("click", () => send("DELETE", `${base}/members/${encodeURIComponent(member.account)}`, void 0, () => again(`${member.name} is out of ${row2.name}. The group needs a new key; make it on the Groups page.`)));
      return el("li", { class: "share" }, el("div", {}, el("span", { class: "share-name" }, member.name), el("span", { class: "detail" }, member.email)), el("span", {}), remove2);
    });
    const candidates = addable(company.value, inGroup.value);
    let chosen = null;
    const choices = candidates.map((member) => {
      const radio = el("input", { id: `join-${member.account}`, type: "radio", name: "join" });
      radio.addEventListener("change", () => chosen = member.account);
      return el("div", { class: "choice" }, radio, el("label", { for: radio.id }, member.name, el("span", { class: "detail" }, member.email)));
    });
    const add = el("button", { type: "button", class: "primary" }, "Add to group");
    add.addEventListener("click", () => chosen === null ? fill(said, refusal("Choose who to add.")) : send("POST", `${base}/members`, { account_id: chosen }, () => again()));
    const name = el("input", { id: "group-rename", type: "text", autocomplete: "off", value: row2.name });
    const rename = el("button", { type: "button" }, "Rename");
    rename.addEventListener("click", () => {
      const wanted = name.value.trim();
      if (wanted !== "" && wanted !== row2.name) {
        send("PATCH", base, { name: wanted }, () => again());
      }
    });
    const keep2 = el("button", { type: "button" }, "Keep it");
    const confirmed = el("button", { type: "button", class: "danger" }, `Delete “${row2.name}”`);
    const confirm = el(
      "dialog",
      { class: "confirm", "aria-labelledby": "delete-group-title" },
      el("h2", { id: "delete-group-title" }, `Delete “${row2.name}”?`),
      el("p", {}, "Records shared with this group stop opening for the people who reached them through it."),
      el("div", { class: "actions" }, keep2, confirmed)
    );
    const remove = el("button", { type: "button", class: "danger" }, "Delete group");
    remove.addEventListener("click", () => confirm.showModal());
    keep2.addEventListener("click", () => confirm.close());
    confirmed.addEventListener("click", () => {
      confirm.close();
      send("DELETE", base, void 0, gone);
    });
    fill(
      main2,
      breadcrumbs(groupCrumbs(groups, row2.name)),
      el("div", { class: "page-head" }, el("div", {}, heading(row2.name), el("p", { class: "lede" }, KIND_WORDS[row2.kind]))),
      said,
      el(
        "section",
        { class: "record-section", "aria-labelledby": "in-group" },
        el("h2", { id: "in-group" }, "People in it"),
        people.length === 0 ? el("p", { class: "muted" }, "Nobody is in this group yet.") : el("ul", { class: "shares" }, ...people),
        candidates.length === 0 ? null : el("fieldset", { class: "choices" }, el("legend", {}, "Add someone"), el("div", { class: "targets" }, ...choices)),
        candidates.length === 0 ? null : add
      ),
      el("section", { class: "record-section", "aria-labelledby": "group-name" }, el("h2", { id: "group-name" }, "Name"), el("div", { class: "field" }, el("label", { for: name.id }, "Group name"), name), rename),
      el("section", { class: "record-section", "aria-labelledby": "group-delete" }, el("h2", { id: "group-delete" }, "Delete"), remove),
      confirm
    );
  }

  // src/groups/group-key.ts
  var KEY_ID_BYTES4 = 16;
  var KEY_BYTES6 = 32;
  async function newGroupKey(company, group, signer, ownerSigning, chain, vouched, pending) {
    const pair = await generatePair("X25519");
    const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
    const raw = pkcs8.slice(pkcs8.length - KEY_BYTES6);
    wipe(pkcs8);
    try {
      const groupPublic = await publicBytes(pair.publicKey);
      const statement = await appendStatement(company, chain.at(-1), { kind: "group", subject: group, agreementPublic: groupPublic, signingPublic: new Uint8Array(0) }, signer);
      await verifyChain(company, ownerSigning, [...chain, statement]);
      const keyId = random(KEY_ID_BYTES4);
      return { statement, keyId, public: groupPublic, ...await wrapToPending(company, keyId, raw, vouched, pending) };
    } finally {
      wipe(raw);
    }
  }
  async function pendingWraps(company, group, member, vouched, listed, pending) {
    const vouchedPublic = vouched.groupKeyOf(group);
    if (vouchedPublic === void 0 || !sameBytes(vouchedPublic, listed.public)) {
      throw new VaultCryptoError("unvouched-recipient");
    }
    if (listed.wrap === null) {
      throw new VaultCryptoError("unknown-key");
    }
    const context2 = { company, wrappedKind: "group", wrappedKeyId: listed.keyId, recipientKind: "member", recipientKeyId: member.keyId };
    const raw = await unwrapSecret(member.agreementPrivate, member.agreementPublic, context2, listed.wrap);
    try {
      return await wrapToPending(company, listed.keyId, raw, vouched, pending);
    } finally {
      wipe(raw);
    }
  }
  async function wrapToPending(company, keyId, raw, vouched, pending) {
    const wraps = [];
    const unvouched = [];
    for (const member of pending) {
      const context2 = { company, wrappedKind: "group", wrappedKeyId: keyId, recipientKind: "member", recipientKeyId: member.keyId };
      try {
        wraps.push({ keyId: member.keyId, wrap: await wrapToVouched(vouched, { kind: "member", account: member.account }, member.agreementPublic, context2, raw) });
      } catch (problem2) {
        if (!(problem2 instanceof VaultCryptoError && problem2.reason === "unvouched-recipient")) {
          throw problem2;
        }
        unvouched.push(member.account);
      }
    }
    return { wraps, unvouched };
  }
  var wrapsBody = (given) => ({
    wraps: given.wraps.map((each) => ({ key_id: hex(each.keyId), wrap: encode(each.wrap) }))
  });
  var groupKeyBody = (made) => ({
    key_id: hex(made.keyId),
    public: encode(made.public),
    wraps: made.wraps.map((each) => ({ key_id: hex(each.keyId), wrap: encode(each.wrap) }))
  });

  // src/groups/groups-view.ts
  var KIND_WORDS2 = { company: "Company", department: "Department", team: "Team" };
  async function drawGroups(main2, held2, pageOf, notice2 = null) {
    const title = heading("Departments and teams");
    const said = el("div", { "aria-live": "polite" }, el("p", { class: "muted", role: "status" }, "Loading the groups."));
    fill(main2, el("div", { class: "page-head" }, title), said);
    const path = companyPath(held2);
    const [groups, chain] = await Promise.all([call("GET", `${path}/groups`, readGroupRows), call("GET", `${path}/statements`, readChain)]);
    if (!groups.ok || !chain.ok) {
      const failed2 = !groups.ok ? groups : chain;
      if (!failed2.ok) {
        fill(said, refusal(loadFailure(failed2)));
      }
      return;
    }
    const vouched = await verifyChain(held2.company, held2.keys.ownerSigningPublic, chain.value.statements).catch(() => null);
    if (vouched === null) {
      fill(said, refusal(CHAIN_REFUSED));
      return;
    }
    const again = (words = null) => void drawGroups(main2, held2, pageOf, words);
    const tree = groupTree({ id: held2.company, name: held2.companyName }, groups.value);
    const waiting = new Map(
      await Promise.all(
        tree.filter((row2) => vouched.groupKeyOf(row2.id) !== void 0).map((row2) => call("GET", `${path}/groups/${encodeURIComponent(row2.id)}/key/pending`, readPending).then((answer) => [row2.id, answer.ok ? answer.value.length : 0]))
      )
    );
    const due = new Set(
      (await Promise.all(
        tree.filter((row2) => vouched.groupKeyOf(row2.id) !== void 0).map((row2) => call("GET", `${path}/groups/${encodeURIComponent(row2.id)}/key`, readRotationDue).then((answer) => answer.ok && answer.value === true ? row2.id : null))
      )).filter((id) => id !== null)
    );
    const items = tree.map((row2) => groupItem(held2, row2, vouched, due.has(row2.id) ? null : waiting.get(row2.id) ?? null, due.has(row2.id), chain.value.statements, again, pageOf));
    fill(
      main2,
      el("div", { class: "page-head" }, el("div", {}, title, el("p", { class: "lede" }, "A record shared with a group opens for everyone in it. A group needs a key before anything can be shared with it."))),
      notice2 === null ? null : refusal(notice2),
      el("section", { class: "record-section", "aria-labelledby": "all-groups" }, el("h2", { id: "all-groups" }, "Groups"), el("ul", { class: "shares" }, ...items)),
      createForm(held2, tree, again)
    );
    title.focus();
  }
  function groupItem(held2, row2, vouched, waiting, rotate, chain, again, pageOf) {
    const said = el("div", { "aria-live": "polite" });
    const keyed = vouched.groupKeyOf(row2.id) !== void 0;
    const give = waiting === null || waiting === 0 ? null : labelled(giveButton(row2.name, waiting));
    give?.addEventListener("click", () => {
      give.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Giving the group's key to the members waiting."));
      void giveAccess(held2, row2.id, vouched).then((words) => {
        give.disabled = false;
        if (words === null) {
          again();
          return;
        }
        fill(said, refusal(words));
      });
    });
    const key = keyed && !rotate ? null : labelled(keyButton(row2.name, rotate));
    key?.addEventListener("click", () => {
      key.disabled = true;
      fill(said, el("p", { class: "muted", role: "status" }, "Making the group's key."));
      void keyGroup(held2, row2.id, chain).then((words) => {
        key.disabled = false;
        if (words === null) {
          again();
          return;
        }
        fill(said, refusal(words));
      });
    });
    return el(
      "li",
      { class: "share group", "data-depth": String(row2.depth) },
      el("div", {}, row2.kind === "company" ? el("span", { class: "share-name" }, row2.name) : el("a", { class: "share-name", href: pageOf(row2.id) }, row2.name), el("span", { class: "detail" }, KIND_WORDS2[row2.kind])),
      el("span", { class: `state ${keyed && !rotate ? "vouched" : "waiting"}` }, rotate ? "Needs a new key" : keyed ? "Has a key" : "No key yet"),
      el("div", { class: "row-actions" }, key ?? give),
      said
    );
  }
  async function keyGroup(held2, id, chain) {
    const path = `${companyPath(held2)}/groups/${encodeURIComponent(id)}/key`;
    const pending = await call("GET", `${path}/pending`, readPending);
    if (!pending.ok) {
      return pending.status === 403 ? "Only an owner or an admin can key a group." : loadFailure(pending);
    }
    const owner = held2.keys.ownerSigningPublic;
    const signer = { privateKey: held2.keys.signingPrivate, publicBytes: held2.keys.signingPublic };
    let made;
    try {
      made = await newGroupKey(held2.company, id, signer, owner, chain, await verifyChain(held2.company, owner, chain), pending.value);
    } catch (problem2) {
      return vouchRefused(problem2 instanceof VaultCryptoError ? problem2.reason : "chain-broken");
    }
    const stated = await call("POST", `${companyPath(held2)}/statements`, ignored, { body: statementBody(chain.length + 1, made.statement) });
    if (!stated.ok) {
      return vouchFailure(stated.status, stated.code, loadFailure(stated));
    }
    const kept = await call("PUT", path, ignored, { body: groupKeyBody(made) });
    return kept.ok ? null : loadFailure(kept);
  }
  async function giveAccess(held2, id, vouched) {
    const path = `${companyPath(held2)}/groups/${encodeURIComponent(id)}/key`;
    const [listed, pending] = await Promise.all([call("GET", path, readGroupKey), call("GET", `${path}/pending`, readPending)]);
    if (!listed.ok || !pending.ok) {
      const failed2 = !listed.ok ? listed : pending;
      return failed2.ok ? null : loadFailure(failed2);
    }
    if (listed.value.wrap === null) {
      return "You don't hold this group's key. Ask someone in the group to give access.";
    }
    let given;
    try {
      given = await pendingWraps(held2.company, id, held2.keys, vouched, listed.value, pending.value);
    } catch (problem2) {
      return vouchRefused(problem2 instanceof VaultCryptoError ? problem2.reason : "chain-broken");
    }
    if (given.wraps.length === 0) {
      return "Nobody has vouched for the waiting members' keys yet. Vouch for them on the Members page first.";
    }
    const kept = await call("POST", `${path}/wraps`, ignored, { body: wrapsBody(given) });
    return kept.ok ? null : loadFailure(kept);
  }
  function createForm(held2, tree, again) {
    let kind = "department";
    let parent = held2.company;
    const name = el("input", { id: "group-name", type: "text", autocomplete: "off" });
    const kinds = ["department", "team"].map((each) => el("button", { type: "button", class: "chip", "aria-pressed": String(each === kind) }, KIND_WORDS2[each]));
    const places = tree.filter((row2) => row2.kind !== "team");
    const parents = el("fieldset", { class: "choices" }, el("legend", {}, "Sits in"), el("div", { class: "targets" }, ...places.map((row2) => placeRow(row2, row2.id === parent, () => parent = row2.id))));
    parents.hidden = true;
    kinds.forEach(
      (button, at2) => button.addEventListener("click", () => {
        kind = at2 === 0 ? "department" : "team";
        kinds.forEach((each, other) => each.setAttribute("aria-pressed", String(other === at2)));
        parents.hidden = kind === "department";
      })
    );
    const submit = el("button", { type: "submit", class: "primary" }, "Make group");
    const said = el("div", { "aria-live": "polite" });
    const form2 = el(
      "form",
      { class: "record-section", novalidate: "" },
      el("h2", {}, "New department or team"),
      el("div", { class: "chips", role: "group", "aria-label": "Kind" }, ...kinds),
      el("div", { class: "field" }, el("label", { for: name.id }, "Name"), name),
      parents,
      submit,
      said
    );
    form2.addEventListener("submit", (event) => {
      event.preventDefault();
      if (name.value.trim() === "") {
        fill(said, refusal("Give the group a name."));
        name.focus();
        return;
      }
      submit.disabled = true;
      const inside = kind === "team" && parent !== held2.company ? { parent } : {};
      const body = { kind, name: name.value.trim(), ...inside };
      void call("POST", `${companyPath(held2)}/groups`, ignored, { body }).then((answer) => {
        submit.disabled = false;
        if (answer.ok) {
          again();
          return;
        }
        fill(said, refusal(createFailure(answer.code, loadFailure(answer))));
      });
    });
    return form2;
  }
  function placeRow(row2, checked, choose) {
    const id = `group-parent-${row2.id}`;
    const radio = el("input", { id, type: "radio", name: "group-parent", ...checked ? { checked: "" } : {} });
    radio.addEventListener("change", choose);
    return el("div", { class: "choice" }, radio, el("label", { for: id }, row2.name, el("span", { class: "detail" }, KIND_WORDS2[row2.kind])));
  }

  // src/sign-in-model.ts
  var isRecord9 = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
  function readMethods(v) {
    if (!isRecord9(v)) {
      return null;
    }
    const { password, passkey: passkey2, provider } = v;
    return typeof password === "boolean" && typeof passkey2 === "boolean" && typeof provider === "boolean" ? { password, passkey: passkey2, provider } : null;
  }
  function readChallenge(v) {
    return isRecord9(v) && typeof v["challenge"] === "string" && v["challenge"] !== "" ? v["challenge"] : null;
  }
  function readSession(v) {
    return isRecord9(v) && typeof v["session"] === "string" && v["session"] !== "" ? v["session"] : null;
  }
  function readPasskeyRequest(v) {
    return isRecord9(v) && typeof v["challenge"] === "string" && typeof v["rp_id"] === "string" ? { challenge: v["challenge"], rpId: v["rp_id"] } : null;
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
  function codeStep(main2, challenge, done, restart) {
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
    fill(main2, view2);
    code.input.focus();
  }
  function signIn(main2, done, notice2 = null) {
    const restart = () => signIn(main2, done);
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
        codeStep(main2, answer.value, done, restart);
        return;
      }
      fill(said, refusal(signInFailure(answer)));
      password.input.focus();
    });
    fill(main2, view2);
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

  // src/crypto/unlock.ts
  var MAX_WRAPS = 2 ** 20;
  var SALT_BYTES3 = 16;
  var MASTER_KEY_BYTES = 32;
  var KEY_ID_BYTES5 = 16;
  var NONCE_BYTES3 = 12;
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
    if (salt.length !== SALT_BYTES3) {
      throw new VaultCryptoError("malformed");
    }
    const wasm = exportsOf(await WebAssembly.instantiate(module2, {}));
    const passwordBytes = utf8(password);
    const passwordLength = Math.max(passwordBytes.length, 1);
    const passwordAt = wasm.alloc(passwordLength);
    const saltAt = wasm.alloc(SALT_BYTES3);
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
      wasm.free(saltAt, SALT_BYTES3);
      wasm.free(outAt, MASTER_KEY_BYTES);
    }
  }
  function associatedData3(context2) {
    if (context2.keyId.length !== KEY_ID_BYTES5) {
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
    if (pin.keyId.length !== KEY_ID_BYTES5) {
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
      const nonce2 = random(NONCE_BYTES3);
      const sealed2 = await crypto.subtle.wrapKey("pkcs8", privateKey, this.#key, {
        name: "AES-GCM",
        iv: nonce2,
        additionalData: associatedData3(context2)
      });
      return concat(Uint8Array.of(FORMAT_VERSION, ALGORITHM.memberKey), context2.keyId, nonce2, new Uint8Array(sealed2));
    }
    /** Seal the company owner's Ed25519 public key under UK, to be pinned in this member's key set; one more wrap. */
    async sealPin(ownerPublic, pin) {
      if (this.#wrapsMade >= MAX_WRAPS) {
        throw new VaultCryptoError("wrap-bound");
      }
      this.#wrapsMade += 1;
      const nonce2 = random(NONCE_BYTES3);
      const sealed2 = await crypto.subtle.wrapKey("raw", ownerPublic, this.#key, {
        name: "AES-GCM",
        iv: nonce2,
        additionalData: pinData(pin)
      });
      return { nonce: nonce2, sealed: new Uint8Array(sealed2) };
    }
    /** Open the pinned owner key; a pin sealed for another member, company or key set is `unauthentic`. */
    async openPin(pin, nonce2, sealed2) {
      if (nonce2.length !== NONCE_BYTES3) {
        throw new VaultCryptoError("malformed");
      }
      try {
        return await crypto.subtle.unwrapKey(
          "raw",
          sealed2,
          this.#key,
          { name: "AES-GCM", iv: nonce2, additionalData: pinData(pin) },
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
      const keyId = reader.take(KEY_ID_BYTES5);
      const nonce2 = reader.take(NONCE_BYTES3);
      const sealed2 = reader.rest();
      if (!sameBytes(keyId, context2.keyId)) {
        throw new VaultCryptoError("key-mismatch");
      }
      const agreement = context2.kind === "agreement";
      try {
        return await crypto.subtle.unwrapKey(
          "pkcs8",
          sealed2,
          this.#key,
          { name: "AES-GCM", iv: nonce2, additionalData: associatedData3(context2) },
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
  var SALT_BYTES4 = 16;
  var KEY_ID_BYTES6 = 16;
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
    const salt = random(SALT_BYTES4);
    const keyId = random(KEY_ID_BYTES6);
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
  function unlockFailure(problem2) {
    if (problem2 instanceof VaultCryptoError && problem2.reason === "unauthentic") {
      return "That unlock password doesn't open your keys. Check it and try again; it is not your sign-in password.";
    }
    if (problem2 instanceof VaultCryptoError && problem2.reason === "kdf-refused") {
      return "Your keys were made with settings this page can't run. Tell your admin.";
    }
    return "This page couldn't open your keys. Reload the page and try again.";
  }
  var loadFailure2 = (failure) => failure.code === "network" ? "The vault didn't answer. Check your connection, then try again." : "The vault couldn't load your keys. Try again in a minute.";
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
    const inputs = companies.map((company, at2) => {
      const input = el("input", { type: "radio", name: "company", id: `company-${at2}`, value: company.id });
      input.checked = at2 === 0;
      return input;
    });
    const row2 = el(
      "fieldset",
      { class: "field choices" },
      el("legend", {}, "Company"),
      ...inputs.flatMap((input, at2) => [el("div", { class: "choice" }, input, el("label", { for: input.id }, companies[at2]?.name ?? ""))])
    );
    return { row: row2, chosen: () => companies.find((company) => company.id === inputs.find((input) => input.checked)?.value) ?? first };
  }
  function unlockAccount(me, company) {
    return el("input", { type: "text", autocomplete: "username", value: `${me.email} (unlock, ${company.name})`, hidden: "", tabindex: "-1" });
  }
  function unlockForm(main2, me, company, stored, choice, done) {
    const password = field("unlock-password", "Unlock password", { type: "password", autocomplete: "current-password", required: "" });
    const submit = el("button", { type: "submit", class: "primary" }, "Unlock");
    const said = el("div", { "aria-live": "polite" });
    const view2 = card2("Unlock your vault", `Your records in ${company.name} open here, in this browser.`, choice, unlockAccount(me, company), password.row, submit, said);
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
        hold({ company: company.id, companyName: company.name, otherCompanies: choice !== null, account: me.account, name: me.name, email: me.email, keys });
        done();
      } catch (problem2) {
        password.input.value = "";
        password.input.setAttribute("aria-invalid", "true");
        fill(said, refusal(unlockFailure(problem2)));
        password.input.focus();
      } finally {
        submit.disabled = false;
      }
    });
    fill(main2, view2);
    password.input.focus();
  }
  function firstUnlockForm(main2, me, company, owner, choice, done) {
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
      unlockAccount(me, company),
      password.row,
      repeated.row,
      submit,
      said
    );
    view2.querySelector("form")?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const problem2 = newPasswordProblem(password.input.value, repeated.input.value, me.email);
      if (problem2 !== null) {
        fill(said, refusal(problem2));
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
        const chained = answer.ok && (owner !== "self" || await beginChain(company, me, made.open));
        const outcome = firstSave(answer.ok ? null : answer.code, chained);
        if (outcome.kind === "failed") {
          if (!answer.ok) {
            fill(said, refusal(loadFailure2(answer)));
          }
          return;
        }
        password.input.value = "";
        repeated.input.value = "";
        if (outcome.kind === "kept") {
          const onward = el("button", { type: "button", class: "primary" }, "Unlock");
          onward.addEventListener("click", () => void unlock(main2, done, signOutHere));
          submit.hidden = true;
          fill(said, refusal(outcome.words), onward);
          onward.focus();
          return;
        }
        hold({ company: company.id, companyName: company.name, otherCompanies: choice !== null, account: me.account, name: me.name, email: me.email, keys: made.open });
        done();
      } catch (problemMaking) {
        fill(said, refusal(unlockFailure(problemMaking)));
      } finally {
        submit.disabled = false;
      }
    });
    fill(main2, view2);
    password.input.focus();
  }
  async function unlock(main2, done, signOut2) {
    signOutHere = signOut2;
    fill(main2, card2("Unlock your vault", "Loading your companies."));
    const [me, companies] = await Promise.all([call("GET", "/api/me", readMe), call("GET", "/api/companies", readCompanies)]);
    if (!me.ok) {
      fill(main2, card2("Unlock your vault", "", refusal(loadFailure2(me))));
      return;
    }
    if (!companies.ok) {
      fill(main2, card2("Unlock your vault", "", refusal(loadFailure2(companies))));
      return;
    }
    const open3 = unlockable(companies.value);
    if (open3.length === 0) {
      fill(
        main2,
        card2(
          "No company yet",
          me.value.platformAdmin ? "You're a platform admin and not a member of any company, so there are no records to unlock." : "You aren't a member of an active company yet. Ask your company's admin to add you."
        )
      );
      return;
    }
    const choice = companyChoice(open3);
    const firstTime = async (company) => {
      const chain = await call("GET", chainPath(company), readChain);
      if (!chain.ok) {
        fill(main2, card2("Unlock your vault", "", choice.row, refusal(loadFailure2(chain))));
        return;
      }
      const owner = ownerPinFor(chain.value, company.role);
      if (owner === "not-yet") {
        const lead = "Your company's owner hasn't opened the vault yet. Once they have, you can set your unlock password here.";
        fill(main2, card2("Waiting for your company's owner", lead, choice.row));
        return;
      }
      firstUnlockForm(main2, me.value, company, owner, choice.row, done);
    };
    const draw2 = async () => {
      const company = choice.chosen();
      const keys = await call("GET", `/api/companies/${encodeURIComponent(company.id)}/keys/me`, readOwnKeys);
      if (keys.ok) {
        unlockForm(main2, me.value, company, keys.value, choice.row, done);
      } else if (keys.status === 404) {
        await firstTime(company);
      } else {
        fill(main2, card2("Unlock your vault", "", choice.row, refusal(loadFailure2(keys))));
      }
    };
    choice.row?.addEventListener("change", () => void draw2());
    await draw2();
  }

  // src/shell-icons.ts
  function setIcon(control2, name) {
    control2.querySelector(":scope > svg.icon")?.remove();
    control2.prepend(icon(name));
  }
  function decorateShell() {
    for (const control2 of document.querySelectorAll("[data-icon]")) {
      const name = control2.dataset["icon"];
      if (isIconName(name)) {
        setIcon(control2, name);
      }
    }
  }

  // src/menu.ts
  var nextMenu = (open3, event) => event === "toggle" ? !open3 : false;
  function installMenu(window2) {
    const document2 = window2.document;
    const button = document2.getElementById("menu-toggle");
    const side = document2.querySelector(".side");
    if (button === null || side === null) {
      return;
    }
    let open3 = false;
    const apply = (event) => {
      open3 = nextMenu(open3, event);
      side.toggleAttribute("data-open", open3);
      button.setAttribute("aria-expanded", String(open3));
    };
    button.addEventListener("click", () => apply("toggle"));
    window2.addEventListener("hashchange", () => apply("navigate"));
    document2.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && open3) {
        apply("escape");
        button.focus();
      }
    });
    document2.addEventListener("click", (event) => {
      if (open3 && event.target instanceof Node && !side.contains(event.target)) {
        apply("outside");
      }
    });
  }

  // src/pane.ts
  function freshPane(view2, make) {
    const pane = make();
    view2.replaceChildren(pane);
    return pane;
  }

  // src/app.ts
  var history = browserHistory(window);
  var notice = null;
  var signedIn = () => bearer() !== null;
  var isUnlocked = () => unlocked() !== null;
  var session = needsSession(signedIn, () => history.current());
  var open2 = every(session, needsUnlock(isUnlocked, () => history.current()));
  var routes = defineRoutes({
    records: { path: "/", guard: open2 },
    newRecord: { path: "/records/new", query: { type: text }, guard: open2 },
    record: { path: "/records/:record", guard: open2 },
    editRecord: { path: "/records/:record/edit", guard: open2 },
    members: { path: "/members", guard: open2 },
    groups: { path: "/groups", guard: open2 },
    group: { path: "/groups/:group", guard: open2 },
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
  var copy = copier({ write: (value) => navigator.clipboard.writeText(value) }, { set: (run, ms) => window.setTimeout(run, ms), clear: (handle) => window.clearTimeout(Number(handle)) }, announce);
  var stops = [];
  var stopView = () => {
    for (const stop of stops.splice(0)) {
      stop();
    }
  };
  var go = (href) => {
    window.location.hash = href;
  };
  function draw(outcome, main2, router) {
    stopView();
    const view2 = freshPane(main2, () => document.createElement("div"));
    document.body.classList.toggle("signed-out", !signedIn() || !isUnlocked());
    showCompany();
    if (outcome.kind !== "matched") {
      heading2(view2, "Not found", "This address names no page of the vault.");
      return;
    }
    switch (outcome.match.name) {
      case "records": {
        const held2 = unlocked();
        if (held2 !== null) {
          void drawList(view2, held2, (record) => router.href("record", { record }), router.href("newRecord", {}), stops);
        }
        break;
      }
      case "record": {
        const held2 = unlocked();
        if (held2 !== null) {
          const record = outcome.match.params.record;
          void drawRecord(view2, held2, record, copy, stops, router.href("records", {}), router.href("editRecord", { record }), () => go(router.href("records", {})));
        }
        break;
      }
      case "newRecord": {
        const held2 = unlocked();
        if (held2 !== null) {
          const opened = (record) => go(router.href("record", { record }));
          void drawNew(view2, held2, outcome.match.query.type, router.href("records", {}), router.href("newRecord", {}), opened, (type) => router.href("newRecord", {}, { type }));
        }
        break;
      }
      case "editRecord": {
        const held2 = unlocked();
        if (held2 !== null) {
          const record = outcome.match.params.record;
          void drawEdit(view2, held2, record, router.href("records", {}), router.href("record", { record }), (saved) => go(router.href("record", { record: saved })));
        }
        break;
      }
      case "members": {
        const held2 = unlocked();
        if (held2 !== null) {
          void drawMembers(view2, held2);
        }
        break;
      }
      case "groups": {
        const held2 = unlocked();
        if (held2 !== null) {
          void drawGroups(view2, held2, (group) => router.href("group", { group }));
        }
        break;
      }
      case "group": {
        const held2 = unlocked();
        if (held2 !== null) {
          void drawGroup(view2, held2, outcome.match.params.group, router.href("groups", {}), () => go(router.href("groups", {})));
        }
        break;
      }
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
  function showCompany() {
    const held2 = unlocked();
    const name = document.getElementById("company");
    const workspace = document.getElementById("workspace");
    const switcher = document.getElementById("switch-company");
    if (name !== null && workspace !== null) {
      name.textContent = held2?.companyName ?? "";
      workspace.hidden = held2 === null;
    }
    const account = document.getElementById("account");
    const person = document.getElementById("account-name");
    const email = document.getElementById("account-email");
    if (account !== null && person !== null && email !== null) {
      person.textContent = held2?.name ?? "";
      email.textContent = held2?.email ?? "";
      account.hidden = held2 === null;
    }
    if (switcher !== null) {
      switcher.hidden = held2?.otherCompanies !== true;
    }
  }
  function switchButton() {
    document.getElementById("switch-company")?.addEventListener("click", () => {
      lock();
      go(`#/unlock?next=${encodeURIComponent("/")}`);
    });
  }
  function themed() {
    const button = document.getElementById("theme");
    const root = document.documentElement;
    const label = () => {
      const words = button?.querySelector("span");
      if (button !== null && words !== null && words !== void 0) {
        const dark = root.dataset.theme === "dark";
        words.textContent = dark ? "Light theme" : "Dark theme";
        setIcon(button, dark ? "sun" : "moon");
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
      stopView();
      void copy.clearNow();
      if (signedIn()) {
        go(`#/unlock?next=${encodeURIComponent(history.current().replace(/^#/, ""))}`);
      }
    });
  }
  var view = document.getElementById("view");
  if (view !== null) {
    decorateShell();
    installMenu(window);
    installTips();
    themed();
    signOutButton();
    switchButton();
    locking();
    void providerReturned().then((refused) => {
      notice = refused;
      const router = new Router(routes, history);
      router.start((outcome) => {
        draw(outcome, view, router);
        mark(outcome);
      });
    });
    document.getElementById("skip")?.addEventListener("click", (event) => {
      event.preventDefault();
      view.focus();
    });
  }
})();
