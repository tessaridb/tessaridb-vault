//! The store's shape, declared by one script that can run on every start.
//!
//! Every definition carries `IF NOT EXISTS`, so running it on a store that already holds the shape changes nothing.
//! A company is its own `company` group; a member's role in the company lives on the `memberships` row, which is
//! queried like a row (by company, by account, by role), while department and team membership is the `in_group`
//! edge of the declared graph `access`, which is what sharing walks (authorization worksheet §13).

use tessari_vault_infrastructure::tessaridb::{Database, Place};

use crate::Result;

/// The definitions, in dependency order.
pub(crate) const SCHEMA: &str = "\
DEFINE GRAPH IF NOT EXISTS access; \
DEFINE TABLE IF NOT EXISTS accounts (account_id string, email string, name string, state string, platform_admin bool, \
created datetime) IN access; \
DEFINE INDEX IF NOT EXISTS by_id ON accounts FIELDS account_id UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_email ON accounts FIELDS email UNIQUE; \
DEFINE TABLE IF NOT EXISTS groups (group_id string, company string, kind string, name string, parent string, state string, \
created datetime) IN access; \
DEFINE INDEX IF NOT EXISTS by_id ON groups FIELDS group_id UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_company ON groups FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_kind ON groups FIELDS kind; \
DEFINE TABLE IF NOT EXISTS memberships (company string, account string, member_role string, state string, \
joined datetime); \
DEFINE INDEX IF NOT EXISTS by_company ON memberships FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_account ON memberships FIELDS account; \
DEFINE EDGE IF NOT EXISTS in_group IN access FROM accounts TO groups; \
DEFINE EDGE IF NOT EXISTS part_of IN access FROM groups TO groups; \
DEFINE TABLE IF NOT EXISTS credentials (account_id string, verifier string, totp string, totp_factor string, \
totp_last_step int, created datetime); \
DEFINE INDEX IF NOT EXISTS by_id ON credentials FIELDS account_id UNIQUE; \
DEFINE SPACE IF NOT EXISTS enrolments; \
DEFINE TABLE IF NOT EXISTS audit_events (company string, seq int, at int, actor string, action string, target string, \
reason string, line string, prev string, digest string); \
DEFINE INDEX IF NOT EXISTS by_company ON audit_events FIELDS company; \
DEFINE TABLE IF NOT EXISTS audit_heads (digest string, seq int); \
DEFINE TABLE IF NOT EXISTS identities (issuer string, subject string, account string, linked datetime); \
DEFINE INDEX IF NOT EXISTS by_identity ON identities FIELDS issuer, subject UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_account ON identities FIELDS account; \
DEFINE SPACE IF NOT EXISTS oidc_pending; \
DEFINE TABLE IF NOT EXISTS passkeys (account string, credential string, public_key string, sign_count int, \
backup_eligible bool, created datetime); \
DEFINE INDEX IF NOT EXISTS by_credential ON passkeys FIELDS credential UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_account ON passkeys FIELDS account; \
DEFINE SPACE IF NOT EXISTS passkey_ceremonies; \
DEFINE SPACE IF NOT EXISTS sessions; \
DEFINE SPACE IF NOT EXISTS sign_ins; \
DEFINE SPACE IF NOT EXISTS throttles; \
DEFINE TABLE IF NOT EXISTS records (record_id string, company string, record_type string, revision int, key_id string, \
author string, state string, created int, updated int); \
DEFINE INDEX IF NOT EXISTS by_id ON records FIELDS record_id UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_author ON records FIELDS company, author; \
DEFINE TABLE IF NOT EXISTS revisions (record string, revision int, author string, author_key string, signature string, \
size int, created int); \
DEFINE INDEX IF NOT EXISTS by_record ON revisions FIELDS record; \
DEFINE TABLE IF NOT EXISTS record_types (type_id string, company string, allowed_roles array, definition string, \
created int); \
DEFINE INDEX IF NOT EXISTS by_id ON record_types FIELDS type_id UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_company ON record_types FIELDS company; \
DEFINE TABLE IF NOT EXISTS files (file_id string, company string, record string, author string, size int, kept string, \
written int, created int); \
DEFINE INDEX IF NOT EXISTS by_id ON files FIELDS file_id UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_record ON files FIELDS record; \
DEFINE BUCKET IF NOT EXISTS record_files MAX 4194304;";

/// Declare the namespace and database of `place` and everything in them that is missing.
///
/// # Errors
///
/// When the store refuses a definition or cannot be reached.
pub async fn migrate(database: &Database, place: &Place) -> Result<()> {
    let namespace = place.namespace();
    let name = place.database();
    // The two names were proven plain when the place was built; they are grammar and cannot be bound.
    let script = format!(
        "DEFINE NAMESPACE IF NOT EXISTS {namespace}; USE NAMESPACE {namespace}; \
         DEFINE DATABASE IF NOT EXISTS {name}; USE DATABASE {name}; {SCHEMA}"
    );
    database.run_unplaced(&script, Vec::new()).await?;
    Ok(())
}
