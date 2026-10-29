//! The store's shape, declared by one script that can run on every start.
//!
//! Every definition carries `IF NOT EXISTS`, so running it on a store that already holds the shape changes nothing.
//! A company is its own `company` group; a member's role in the company lives on the `memberships` row, which is
//! queried like a row (by company, by account, by role); the company's row also names the service-key epoch its
//! records vault is keyed under (`key_epoch`) and whether a rotation is moving it
//! (`key_moving`), which every other group carries at their defaults, while department and team membership is the `in_group`
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
DEFINE FIELD IF NOT EXISTS key_epoch ON groups TYPE int DEFAULT 1; \
DEFINE FIELD IF NOT EXISTS key_moving ON groups TYPE bool DEFAULT false; \
DEFINE TABLE IF NOT EXISTS memberships (company string, account string, member_role string, state string, \
joined datetime); \
DEFINE INDEX IF NOT EXISTS by_company ON memberships FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_account ON memberships FIELDS account; \
DEFINE EDGE IF NOT EXISTS in_group IN access FROM accounts TO groups; \
DEFINE EDGE IF NOT EXISTS part_of IN access FROM groups TO groups; \
DEFINE TABLE IF NOT EXISTS company_drops (company string, state string, requested_by string, requested_at int, \
signature string, confirmed_by string, drop_after int); \
DEFINE TABLE IF NOT EXISTS credentials (account_id string, verifier string, totp string, totp_factor string, \
totp_last_step int, created datetime); \
DEFINE INDEX IF NOT EXISTS by_id ON credentials FIELDS account_id UNIQUE; \
DEFINE FIELD IF NOT EXISTS key_epoch ON credentials TYPE int DEFAULT 1; \
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
author string, state string, share_version int, rotate bool, created int, updated int); \
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
DEFINE BUCKET IF NOT EXISTS record_files MAX 4194304; \
DEFINE TABLE IF NOT EXISTS member_keys (company string, account string, key_id string, agreement_public string, \
signing_public string, kdf_salt string, kdf_memory int, kdf_passes int, kdf_lanes int, created int); \
DEFINE INDEX IF NOT EXISTS by_company ON member_keys FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_key ON member_keys FIELDS company, key_id UNIQUE; \
DEFINE TABLE IF NOT EXISTS chains (company string, owner_signing string, length int, created int); \
DEFINE TABLE IF NOT EXISTS statements (company string, seq int, bytes string, signature string, author string, \
created int); \
DEFINE INDEX IF NOT EXISTS by_company ON statements FIELDS company; \
DEFINE TABLE IF NOT EXISTS group_keys (company string, group_id string, key_id string, group_public string, \
rotate bool, created datetime); \
DEFINE INDEX IF NOT EXISTS by_key ON group_keys FIELDS company, key_id UNIQUE; \
DEFINE TABLE IF NOT EXISTS shares (company string, record string, target_kind string, target string, access string, \
recipient string, by_account string, created int); \
DEFINE INDEX IF NOT EXISTS by_record ON shares FIELDS record; \
DEFINE INDEX IF NOT EXISTS by_target ON shares FIELDS company, target; \
DEFINE TABLE IF NOT EXISTS recovery_keys (company string, key_id string, public string, created datetime); \
DEFINE TABLE IF NOT EXISTS escrows (company string, account string, recovery_key_id string, created int); \
DEFINE TABLE IF NOT EXISTS recovery_notices (company string, account string, by_account string, kind string, \
created int); \
DEFINE INDEX IF NOT EXISTS by_member ON recovery_notices FIELDS company, account; \
DEFINE TABLE IF NOT EXISTS recovery_requests (request_id string, company string, account string, handoff_public string, created int, \
expires int, handed_by string, redeemed int, consumed int); \
DEFINE INDEX IF NOT EXISTS by_company ON recovery_requests FIELDS company;";

/// Declare the database of `place` and everything in it that is missing.
///
/// The namespace is the store owner's to declare, with the server's account as an editor of it alone: declaring a
/// namespace takes authority over the whole store, which the server never holds.
///
/// # Errors
///
/// When the store refuses a definition or cannot be reached, including when the namespace does not exist.
pub async fn migrate(database: &Database, place: &Place) -> Result<()> {
    let namespace = place.namespace();
    let name = place.database();
    // The two names were proven plain when the place was built; they are grammar and cannot be bound.
    let script = format!(
        "USE NAMESPACE {namespace}; DEFINE DATABASE IF NOT EXISTS {name}; USE DATABASE {name}; {SCHEMA}"
    );
    database.run_unplaced(&script, Vec::new()).await?;
    Ok(())
}
