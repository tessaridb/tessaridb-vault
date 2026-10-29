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

[tessaridb.com](https://tessaridb.com) · [TessariDB](https://github.com/tessaridb/tessaridb)

</div>

> [!NOTE]
> **In development, not released.** Apart from the [Status](#status) section, this page describes what the
> project is being built to do, not a working system. The first release will be `0.0.1`.

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
- **Deleting a company destroys its key**, so no later copy of its data opens.
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
   service's memory and see everything in item 5. Without your unlock password they still cannot decrypt a record.
4. **Your company's owners and admins.** Owners hold the company's recovery key, so they can restore any member's
   access and can therefore read every record shared with members of that company. Admins hold the keys of the
   company's departments and teams, so they can read every record shared with a department, a team or the whole
   company — not records shared only with individual people. Every member is shown this.
5. **Metadata.** The server sees who belongs to which company, department and team, who shares which record with whom,
   record types, sizes, counts and timestamps, and when records are fetched. Names of records are encrypted; their
   existence is not.
6. **Someone who already read a secret.** Removing a person stops their future access and rotates the record's key.
   It cannot make them forget a value they saw. Change the secret at its source.
7. **A restored old copy.** Someone who can write to the database can put back an old version of a record. Your
   browser refuses a version older than one it has already seen, but cannot detect an old version it has never seen
   newer of.
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
| Release | none yet; the first will be `0.0.1` |
| Built and tested | the workspace; the unlock-key derivation — Argon2id with bounds checked before any work (memory at least 47 104 KiB, passes 1–10, lanes 1–8), built for the browser as WebAssembly and checked against an independent vector; the server's companies, members, departments and teams, every route behind one access decision on the caller's standing, with sessions and rate limits held in TessariDB and a live suite that calls each route unsigned, as a lower role and as another company; own sign-in: a member enrols through a single-use link an admin issues, setting a password (Argon2id under a pepper derived from the service key) and a TOTP authenticator, then signs in with the password and one code, with attempts backed off per address, per address-and-password pair and per network address, never locking the account; sign-in through an OpenID Connect issuer (PKCE, `state`, `nonce`, `iss` and the ID token checked), where a signed-in member links their identity at the issuer and a return completes only for the browser or the account that began it; passkeys (Web Authentication Level 3, user verification required), registered by a signed-in member and used to sign in without naming an account first; the company audit trail: every admin change is recorded with who, what, to whom, when and why in the same transaction as the change, so a change whose record cannot be written does not happen, and a chain over each company's events shows where a row was altered; the browser's encryption module, with no third-party code: each revision of a record sealed under a key used once and bound to its company, record, revision and key, keys wrapped to each recipient with HPKE (RFC 9180, checked against the standard's published vectors), the member's private keys wrapped under the key derived from the unlock password, and the signature chain from the company owner's key that decides which keys a browser encrypts to and which authors it trusts; the record content it seals: eleven built-in record types and fifteen field types in named sections, one-time password codes (RFC 6238), SSH Ed25519 keys generated and imported in OpenSSH's formats with their fingerprints, and files encrypted in 64 KiB chunks up to 256 MiB, refused if a chunk is reordered, dropped or added; records on the server: each revision's sealed body kept in the company's own TessariDB vault, which opens only for a request presenting that company's key (derived per request, never stored), read and revised by its author and by the members it is shared with, record types a company defines with the roles that may use each, a revision written only over the one it was read at, every create, read and revision on the company's trail before anything is opened, and no record body found by a scan of a backup or the store directory, nor a value sealed by the browser's module found in the body the server holds or in the audit trail; encrypted files on a record, uploaded by its author or an editor in 4 MiB parts in order and read back in ranges only when complete, a file up to 64 KiB kept in the company's vault and a larger one in a TessariDB bucket, each upload and read on the company's trail; members' key sets on the server, one per member per company, the public half listed to the company and the private half wrapped in the browser and kept in the company's vault, signed only under the caller's own key; and the vouching chain, begun by an owner who pins the owner key and extended one statement at a time by owners and admins; group keys, whose private half an admin's browser wraps only to members inside the group's reach (the company, a department with its teams, or a team), each member reading back only their own wrap, with a list of members still waiting for one; sharing a record with a member, a team, a department or the whole company, each share written in one transaction with the record key wrapped to that member's or group's registered key, read by everyone it reaches and also revised, shared on and given files by those it gives edit access, every revision keeping a wrap for exactly the record's author and shares; removing a share, which stops the member it reached at their next request, takes their wrap off the latest revision and requires the next revision to be sealed under a new record key; taking a member out of a team or department, which stops them at their next request, takes their wrap off the group's key and marks that key, its department's and every record shared through them for rotation, an admin then keying the group again and its shares naming the new key; a member leaving the company, which marks the company's key, every key of their groups and every record they wrote or could open the same way; and organisation recovery: an owner keeps the company's recovery key, wrapped only to owners and vouched for in the chain by the owner key alone, every member's browser escrows its keys to it, and a member who lost the unlock password asks with a one-time key their browser makes, an owner's browser hands their keys over to it, and they redeem it once within 72 hours and wrap their keys under a new unlock password; an admin, another company and the platform admin are refused, the store's administrator finds the escrow sealed, and every read of a member's escrow and every recovery of them is shown to that member |
| Being built | the web application, served by the vault itself: joining with the link an admin issues, signing in, unlocking a company's keys (the sidebar names the company), the records list with search and filters by type, by author and by the person's company, departments and teams, opening, creating, editing and deleting records, sharing a record with a member or a group and taking the share back, a members page where an owner or admin adds someone with a role and gets the join link to send them, changes a member's role, suspends, resumes or removes them, and vouches for a member's key after comparing its fingerprint, departments and teams, each with its own page for its key, its people, its name and deleting it, the company's trail, newest first with who did what to whom and why, filtered by area and checked end to end, recovery — the owner holding the company owner key makes the recovery key once, every member's keys are kept for it at their next unlock, a member who forgot their unlock password asks from the unlock card and reads a code to the owner who hands their keys back, then sets a new unlock password, and every reach of a member's keys is shown to them after they unlock — and an account page that adds a passkey |
| Designed, not yet built | recovery in the web application, files in TessariDB S3, and the audit events of sharing |
| Licence | BUSL-1.1 (see [Licence](#licence)) |

## In a container

The `Dockerfile` builds an image holding only the `tessaridb-vault` server, with the web application inside it. Its
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
