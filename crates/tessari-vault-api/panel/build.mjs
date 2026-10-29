// Renders the web application's source into the committed assets under ../assets/.
//
// Building or running the server never comes through here: the output is committed and embedded by include_bytes!, so
// `cargo build`, `docker build` and a release need no Node. The build is deterministic (run it twice and the second
// run changes no byte), which is what `verify.sh` checks. EVERY file under ../assets/ is written by this script.

import { build } from "esbuild";
import { copyFile, readFile, writeFile } from "node:fs/promises";

const ASSETS = new URL("../assets/", import.meta.url);
const STATIC = new URL("./assets/", import.meta.url);
const STYLES = ["tokens.css", "base.css", "shell.css", "forms.css"];

/** Nothing the page loads may come from off this server: the vault must work on a network with no route out. */
function local(what, text) {
  if (/\b(src|href)\s*=\s*["']?(https?:)?\/\//i.test(text) || /url\(\s*["']?(https?:)?\/\//i.test(text)) {
    throw new Error(`${what} reaches for something off this server`);
  }
}

await build({
  entryPoints: [new URL("./src/app.ts", import.meta.url).pathname],
  outfile: new URL("app.js", ASSETS).pathname,
  bundle: true,
  format: "iife",
  platform: "browser",
  target: "es2022",
  charset: "utf8",
  legalComments: "inline",
  logLevel: "warning",
});

const script = await readFile(new URL("app.js", ASSETS), "utf8");
local("the emitted script", script);
// `Function` compiles the source without running it, so a script that would be a SyntaxError fails the build instead.
try {
  new Function(script);
} catch (failure) {
  throw new Error(`the emitted script does not parse: ${failure.message}`);
}
console.log(`app.js       ${script.length} bytes, parses`);

const styles = [];
for (const name of STYLES) {
  const text = await readFile(new URL(`./styles/${name}`, import.meta.url), "utf8");
  local(name, text);
  styles.push(text);
}
await writeFile(new URL("app.css", ASSETS), styles.join("\n"));
console.log(`app.css      joined from ${STYLES.join(", ")}`);

const page = await readFile(new URL("index.html", STATIC), "utf8");
local("index.html", page);
await copyFile(new URL("index.html", STATIC), new URL("index.html", ASSETS));
console.log("index.html   copied");

// The product mark has one home, the repository's logo directory; the page serves that file rather than a copy.
const MARK = new URL("../../../assets/logo/tessaridb-vault-mark.svg", import.meta.url);
local("the product mark", await readFile(MARK, "utf8"));
await copyFile(MARK, new URL("favicon.svg", ASSETS));
console.log("favicon.svg  copied from assets/logo/tessaridb-vault-mark.svg");

// The vault's own WebAssembly module (Argon2id for the unlock password), built by `prebuild` from tessari-vault-crypto.
const CRYPTO = new URL("../../../target/wasm32-unknown-unknown/release/tessari_vault_crypto.wasm", import.meta.url);
const module = await readFile(CRYPTO);
await WebAssembly.compile(module);
await copyFile(CRYPTO, new URL("vault-crypto.wasm", ASSETS));
console.log(`vault-crypto.wasm ${module.length} bytes, compiles`);
