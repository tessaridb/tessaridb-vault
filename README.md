<div align="center">

<img src="assets/logo/tessaridb-vault-mark-256.png" alt="" width="112" height="112">

# TessariDB Vault

**A password manager for companies, with every record encrypted in the browser and every piece of state in TessariDB.**

Companies, departments and teams share passwords, keys, notes and small files as end-to-end encrypted records.
The server and the database store and move ciphertext; neither can read a record. Written in Rust, with the web
application built into the server binary.

[![status](https://img.shields.io/badge/status-in%20development-6B5FD1?style=flat-square)](#status)
[![licence](https://img.shields.io/badge/licence-BUSL--1.1-6B5FD1?style=flat-square)](LICENSE)
[![rust](https://img.shields.io/badge/rust-1.98%2B-6B5FD1?style=flat-square)](#building-and-checking)

[vault.tessaridb.com](https://vault.tessaridb.com) · [TessariDB](https://github.com/tessaridb/tessaridb)

</div>

> [!NOTE]
> **`0.0.2` is the current release.** The [Status](#status) section lists what is built and tested, and what is
> designed but not yet built.

## What it is for

- **Shared by structure.** A record is shared with a person, a team, a department or the whole company, and a change
  of team changes who can read it without rewriting the record.
- **Encrypted before it leaves the browser.** Contents, titles, field names and files are sealed on the device that
  writes them; the unlock password is separate from the sign-in and never leaves the browser.
- **A second layer in the database.** Each company's ciphertext is kept in its own TessariDB vault, opened by a key
  the database never stores, so the store's own vault passphrase and its root account open none of it.
- **Recovery the company can see.** The company's owners can restore a member's access, and every member is told
  who can recover and read what.
- **State in TessariDB.** Companies, people, groups, records, files, sessions, throttles and the audit trail are that
  database's records, vaults, spaces and buckets, not a second store kept beside it.

## The promises it is being designed to keep

These are design commitments. None is claimed until a test backs it, and the [Status](#status) section says which
exist.

- **Ciphertext at rest, everywhere.** A record written through the browser appears in the store directory, a backup
  and the audit trail only as ciphertext, checked by scanning the raw bytes.
- **Writing a row adds no reader.** A browser encrypts to, and trusts revisions from, only keys that a signature chain
  from the company owner's key vouches for.
- **Tampering aborts.** A moved, altered or unsigned revision is refused, and so is a revision older than one the
  browser has already seen.
- **Every bundle served is on the record**, and one that cannot be recorded is not served.
- **Removing a person takes effect at their next request**, and the record keys they held are rotated.
- **Deleting a company leaves nothing of it in the live store** but its audit trail, which ends with the deletion. A
  backup taken before the deletion opens at the store layer only until the service-key epoch the company was keyed
  under leaves the key file at the next completed rotation; its record bodies stay sealed end to end throughout.
- **The server writes no core file**: before it reads a key it sets its core size limit to zero and, on Linux, marks
  itself undumpable, so a crash or a debugger of the same user does not take the keys it derives out of memory.

### What it does not protect against

1. **An unlocked browser and the device it runs on.** While your vault is unlocked, the keys are in that tab's memory.
   Malware on the device, or a browser extension allowed to read the page, can read what you can.
2. **The code the server delivers.** The web application is loaded from the server you sign in to. Whoever controls
   that server can serve a modified application that captures your unlock password. It ships no third-party code, sets
   a strict content security policy and will publish the hash of every release's assets so that a change can be
   noticed, but a web client cannot prove to you that it is honest.
3. **The server host.** Someone with control of the host can read the database-side keys and session tokens in the
   service's memory, the sign-in passwords people send while they hold it (a request's bytes stay in memory until
   later requests reuse it), and everything in item 5. Without your unlock password, which never reaches the
   server, they still cannot decrypt a record.
4. **Your company's owners and admins.** Owners hold the company's recovery key, so they can restore any member's
   access and can therefore read every record shared with members of that company. Admins hold the keys of the
   company's departments and teams, so they can read every record shared with a department, a team or the whole
   company — not records shared only with individual people. Every member is shown this.
5. **Metadata.** The server sees who belongs to which company, department and team, who shares which record with whom,
   record types, sizes, counts and timestamps, and when records are fetched. Names of records are encrypted; their
   existence is not.
6. **Someone who already read a secret.** Removing a person stops their future access and rotates the record's key.
   It cannot make them forget a value they saw. Change the secret at its source. Their key also keeps counting in
   the company's key chain until an owner or admin ends it on the Members page; until then, a server under an
   attacker's control that holds their private key can store revisions in their name.
7. **A restored old copy, and a replaced key.** Someone who can write to the database can put back an old version of
   a record; this version of the browser does not yet remember the newest revision it has seen, so it cannot tell.
   Each revision names the position in the company's key chain its author signed at and is checked with the key the
   chain vouched for them there, which keeps a member's revisions readable after their key is replaced. The cost: a
   server under an attacker's control, given that member's old private key, can store a revision under the old key
   claiming a position from before the replacement, on a record the member could write. An honest server refuses it.
8. **A weak unlock password.** Anyone holding the database and the server's key file can try to guess your unlock
   password offline. Each guess costs a memory-hard key derivation, but a short or common password still falls.
9. **Your sign-in provider.** A compromised identity provider can sign you in, but it cannot unlock your vault: the
   unlock password is separate and never leaves your browser.
10. **Files kept in an external object store.** When a deployment stores files in TessariDB S3 instead of the
    database, those files carry the browser's encryption but not the database's second layer, and their removal
    follows that store's own lifecycle.
11. **Timing and traffic analysis**, and **future quantum computers** against today's ciphertext (the key exchange is
    classical X25519 in this version; the format names its algorithms so it can move).

## Status

| | |
|---|---|
| Release | `0.0.2` |
| Built and tested | the workspace; the unlock-key derivation — Argon2id with bounds checked before any work (memory at least 47 104 KiB, passes 1–10, lanes 1–8), built for the browser as WebAssembly and checked against an independent vector; the server's companies, members, departments and teams, every route behind one access decision on the caller's standing, with sessions and rate limits held in TessariDB and a live suite that calls each route unsigned, as a lower role and as another company; own sign-in: a member enrols through a single-use link an admin issues, setting a password (Argon2id under a pepper derived from the service key) and a TOTP authenticator, then signs in with the password and one code, with attempts backed off per address, per address-and-password pair and per network address, never locking the account; sign-in through an OpenID Connect issuer (PKCE, `state`, `nonce`, `iss` and the ID token checked), where a signed-in member links their identity at the issuer from their account page and a return, a sign-in or a link, completes only for the browser that began it; passkeys (Web Authentication Level 3, user verification required), registered by a signed-in member and used to sign in without naming an account first, listed and removed only by that member, each addition and removal on the trail of every company they belong to; the company audit trail: every admin change is recorded with who, what, to whom, when and why in the same transaction as the change, so a change whose record cannot be written does not happen, and a chain over each company's events shows where a row was altered; the browser's encryption module, with no third-party code: each revision of a record sealed under a key used once and bound to its company, record, revision and key, keys wrapped to each recipient with HPKE (RFC 9180, checked against the standard's published vectors), the member's private keys wrapped under the key derived from the unlock password, and the signature chain from the company owner's key that decides which keys a browser encrypts to and which authors it trusts, each revision checked with the key the chain vouched for its author at the chain position the revision names, and stored by the server only at the chain's current length; the record content it seals: twelve built-in record types and fifteen field types in named sections, one-time password codes (RFC 6238), SSH Ed25519 keys generated and imported in OpenSSH's formats with their fingerprints and both halves downloadable, and files encrypted in 64 KiB chunks up to 256 MiB, refused if a chunk is reordered, dropped or added; records on the server: each revision's sealed body kept in the company's own TessariDB vault, which opens only for a request presenting that company's key (derived per request, never stored), read and revised by its author and by the members it is shared with, record types a company defines with the roles that may use each, a revision written only over the one it was read at, every create, read and revision on the company's trail before anything is opened, and no record body found by a scan of a backup or the store directory, nor a value sealed by the browser's module found in the body the server holds or in the audit trail; encrypted files on a record, uploaded by its author or an editor in 4 MiB parts in order and read back in ranges only when complete, a file up to 64 KiB kept in the company's vault and a larger one in a TessariDB bucket or in TessariDB S3, sent only when the record is saved and deleted again if the save fails, deleted only by its uploader, each upload, read and deletion on the company's trail, and a company drop removes its S3 objects with their metadata; members' key sets on the server, one per member per company, the public half listed to the company and the private half wrapped in the browser and kept in the company's vault, signed only under the caller's own key; and the vouching chain, begun by an owner who pins the owner key and extended one statement at a time by owners and admins, the server keeping a statement only once the chain with it verifies as every browser verifies it, the Members page giving admins and later owners the standing to vouch, taking it from a member, and ending the key of someone who left while what they wrote stays readable; group keys, whose private half an admin's browser wraps only to members inside the group's reach (the company, a department with its teams, or a team), each member reading back only their own wrap, with a list of members still waiting for one; sharing a record with a member, a team, a department or the whole company, each share written in one transaction with the record key wrapped to that member's or group's registered key, read by everyone it reaches and also revised, shared on and given files by those it gives edit access, every revision keeping a wrap for exactly the record's author and shares; removing a share, which stops the member it reached at their next request, takes their wrap off the latest revision and requires the next revision to be sealed under a new record key; taking a member out of a team or department, which stops them at their next request, takes their wrap off the group's key and marks that key, its department's and every record shared through them for rotation, an admin then keying the group again and its shares naming the new key; a member leaving the company, which marks the company's key, every key of their groups and every record they wrote or could open the same way; and organisation recovery: an owner keeps the company's recovery key, wrapped only to owners and vouched for in the chain by the owner key alone, and gives it to an owner made later from their own copy; an owner leaving or losing the role retires it, and the owner holding the owner key makes the next, which drops every escrow made to the old one so each member escrows again at their next unlock; every member's browser escrows its keys to it, and a member who lost the unlock password asks with a one-time key their browser makes, an owner's browser hands their keys over to it, and they redeem it once within 72 hours and wrap their keys under a new unlock password; an admin, another company and the platform admin are refused, the store's administrator finds the escrow sealed, and every read of a member's escrow and every recovery of them is shown to that member; and the service key as a ring of numbered epochs: each company vault, sign-in verifier and TOTP seed names the epoch it was made under and opens under that epoch, `tessaridb-vault rotate` moves every company vault and seed to the newest one, a verifier moves at its member's next sign-in, and the counts of what still rests on each epoch say when an old one may go; and deleting a company: an owner asks with a statement signed by their own key, the platform admin confirms, the company is then read-only for seven days in which an owner may cancel, and `tessaridb-vault drop-due` drops its vault and removes its rows, group edges, file parts and S3 objects in one transaction, its trail keeping the deletion |
| The web application | served by the vault itself: joining with the link an admin issues, signing in, unlocking a company's keys (the sidebar names the company), the records list with search and filters by type, by author and by the person's company, departments and teams, opening, creating, editing and deleting records, sharing a record with a member or a group and taking the share back, a members page where an owner or admin adds someone with a role and gets the join link to send them, changes a member's role, suspends, resumes or removes them, and vouches for a member's key after comparing its fingerprint, gives admins the standing to vouch and ends the keys of people who left, departments and teams, each with its own page for its key, its people, its name and deleting it, the company's trail, newest first with who did what to whom and why, filtered by area and checked end to end, recovery — the owner holding the company owner key makes the recovery key once, an owner gives it to owners made after it, every member's keys are kept for it at their next unlock, a member who forgot their unlock password asks from the unlock card and reads a code to the owner who hands their keys back, then sets a new unlock password, and every reach of a member's keys is shown to them after they unlock — an account page that adds a passkey, lists the person's passkeys and removes one, and links the organisation sign-in, a company page where an owner asks for the company's deletion, typing its name, giving a reason and signing in the browser, and cancels it while it waits, and the platform admin's companies page, which creates a company and shows its owner's join link once, suspends and resumes one, and confirms a deletion an owner asked for after its name and a reason are typed |
| Designed, not yet built | the audit events of sharing |
| Licence | BUSL-1.1 (see [Licence](#licence)) |

## Running

The server reads its settings from the environment and refuses to start, naming the variable, when one it needs is
missing or wrong.

| Variable | Required | Default | What it is |
|---|---|---|---|
| `TESSARIDB_VAULT_STORE` | yes | | the TessariDB node, `host:port` |
| `TESSARIDB_VAULT_STORE_USER`, `TESSARIDB_VAULT_STORE_PASSWORD` | yes | | the server's own database account, never the store's owner |
| `TESSARIDB_VAULT_STORE_AUTHORITY` | | | PEM certificates for a store reached over TLS; without it the store is reached only at `127.0.0.1` or `[::1]` |
| `TESSARIDB_VAULT_SERVICE_KEY_FILE` | yes | | the service key file, readable by the server's user alone: one key (64 hexadecimal characters, `openssl rand -hex 32`), or one `<epoch> <key>` line per epoch from 1 up, ascending, the highest current |
| `TESSARIDB_VAULT_NAMESPACE`, `TESSARIDB_VAULT_DATABASE` | | `vault`, `main` | where in the store the vault keeps everything |
| `TESSARIDB_VAULT_LISTEN` | | `127.0.0.1:8080` | the address the server listens on |
| `TESSARIDB_VAULT_PUBLIC_URL` | | | the https origin people reach the server at (http only to `localhost` or a loopback address); passkeys are offered only when it is set |
| `TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL`, `TESSARIDB_VAULT_PLATFORM_ADMIN_NAME` | both or neither | | the platform admin, who creates companies |
| `TESSARIDB_VAULT_S3_ENDPOINT`, `TESSARIDB_VAULT_S3_BUCKET`, `TESSARIDB_VAULT_S3_ACCESS_KEY`, `TESSARIDB_VAULT_S3_SECRET_KEY` | all four or none | | store new large files in TessariDB S3 instead of the database bucket; endpoint must be https, or http to localhost/loopback |
| `TESSARIDB_VAULT_S3_REGION` | | `us-east-1` | the S3 region |
| `TESSARIDB_VAULT_OIDC_ISSUER`, `TESSARIDB_VAULT_OIDC_CLIENT_ID`, `TESSARIDB_VAULT_OIDC_REDIRECT` | all three or none | | sign-in through an OpenID Connect issuer |

`tessaridb-vault` serves; `tessaridb-vault enrolment-link` prints a single-use enrolment token for the platform
admin, valid for 24 hours, and exits (the platform admin joins with it at `/#/join?token=…`, then creates companies
from Manage companies); `tessaridb-vault rotate` moves everything to the service key's newest epoch
and exits; `tessaridb-vault drop-due` carries out the company deletions whose seven-day wait has passed and exits
(run it daily, then rotate the service key); `tessaridb-vault health` asks the running server whether it answers.

### Rotating the service key

1. Add the next epoch to the key file: a file holding one key becomes `1 <that key>` with `2 <new key>` below it.
   Every server must read the new file, so restart them all before the next step.
2. Run `tessaridb-vault rotate` in a quiet window. While a company's vault moves, its requests are refused with
   `409 keys_moving` and present no key: after three wrong keys in a row the store refuses every vault read on the
   node for a while, and this keeps a racing request from spending them. A run cut short is finished by running it
   again.
3. The command prints what still rests on each epoch. Company vaults and TOTP seeds move at once; a sign-in
   password moves when its member next signs in. A member whose password still rests on a removed epoch cannot sign
   in; the platform admin resets their sign-in with `POST /api/accounts/{account}/sign-in/reset`, which is recorded
   on the trail of every company they belong to and answers a new single-use enrolment link (their records stay
   sealed under their own unlock password). A company's admin cannot: a sign-in belongs to the person, not to one
   company.
4. When nothing rests on the old epoch, remove its line and restart. A backup of the key file that still holds that
   epoch still opens what it opened, so expire those backups too: a database backup taken before the rotation stays
   readable at the store layer for as long as one of them exists, while record bodies stay sealed by the browser
   throughout.

## In a container

The `Dockerfile` builds the image published as `tessaridb/vault`, holding only the `tessaridb-vault` server with the web application inside it. Its
state lives in a TessariDB node beside it, and `compose.yaml` runs the two together:

```sh
cp .env.example .env        # set the store owner's password, the vault account's password and the platform admin
docker compose up -d --wait
docker compose run --rm vault enrolment-link
```

The database (`tessaridb/tessaridb:0.39.6`; the server needs 0.39.3 or later, and 0.39.4 is withdrawn) declares its
owner from `.env` on first start, and its port is not published: the server runs in the database container's
network namespace and reaches it at `127.0.0.1`, the only place the server speaks to a store without TLS. A one-shot
`vault-account` step (`docker/vault-account.sh`) signs in as that owner once to declare the `vault` namespace and the
server's own account, an editor of that namespace alone; the server signs in as it and never as the owner. A second
one-shot step (`docker/service-key.sh`) writes the service key into the `vault-keys` volume, readable by the server
alone, and leaves an existing key untouched. Back that volume up apart from the database's: a database restored
without its service key cannot be opened. The server (`8080`) is published on `127.0.0.1`; put a TLS-terminating
proxy in front and set `VAULT_PUBLIC_URL` to its https address before exposing it. The image is distroless and runs
as the non-root user `65532`; it carries no credential, and the server refuses to start without the values it needs.
Its health check runs `tessaridb-vault health`, which asks the server in the container whether it answers, so
`docker compose up --wait` returns once the server serves.

## Building and checking

A Cargo workspace on Rust 1.98 (edition 2024).

```sh
cargo build
cargo test --workspace --tests
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo deny check   # licences, advisories, sources
```

The live suite runs the server against a real TessariDB node it starts itself, from an engine binary at 0.39.3
or later:

```sh
TESSARIDB_BIN=/path/to/tessaridb cargo test -p tessari-vault-api --test live -- --ignored
```

The parsers that read a request before anyone has signed in (passkey responses, identifiers, addresses,
base64url) have fuzz targets in `fuzz/`, a workspace of their own, run by hand when one of those parsers changes:

```sh
cd fuzz
cargo +nightly fuzz run passkey -- -max_total_time=600
cargo +nightly fuzz run sign_in_text -- -max_total_time=600
```

## Branches

`dev` is where development happens. `main` will carry releases and move only by merging `dev` at a release, with
each release tagged there.

## Licence

TessariDB Vault is licensed under the [Business Source License 1.1](LICENSE). Production use is free, including use
by and within a commercial company, and keeping your own organisation's secrets with it is not restricted. Two things
need a commercial licence from the Licensor:
- providing TessariDB Vault to third parties as a hosted or managed service for storing, sharing or managing their
  passwords, credentials, keys or other secrets;
- selling, reselling, renting or sublicensing it.

On the Change Date in [`LICENSE`](LICENSE) each version converts to the Apache License, Version 2.0. For other
arrangements, write to licensing@tessaridb.com.
