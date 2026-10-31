//! The store's shape, declared by one script that can run on every start.
//!
//! Every definition carries `IF NOT EXISTS`, so running it on a store that already holds the shape changes nothing.
//! Native record identities are used for entities; relationships use `record` and explicit `DEFINE FIELD` constraints.
//! Companies carry lifecycle flags and the service-key epoch their records vault is keyed under (`key_epoch`) and
//! whether a rotation is moving it (`key_moving`), while department and team membership is the `in_group` edge of
//! the declared graph `access`, which is what sharing walks (authorization worksheet §13).

use tessari_vault_infrastructure::tessaridb::{Database, Place};

use crate::Result;

/// The definitions, in dependency order.
pub(crate) const SCHEMA: &str = "\
DEFINE GRAPH IF NOT EXISTS access; \
DEFINE TABLE IF NOT EXISTS accounts (email string, name string, is_active bool, platform_admin bool, created datetime) IN access; \
DEFINE INDEX IF NOT EXISTS by_email ON accounts FIELDS email UNIQUE; \
DEFINE TABLE IF NOT EXISTS companies (name string, is_active bool, is_suspended bool, is_dropping bool, key_epoch int, key_moving bool, created datetime) IN access; \
DEFINE INDEX IF NOT EXISTS by_name ON companies FIELDS name UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_is_active ON companies FIELDS is_active; \
DEFINE TABLE IF NOT EXISTS groups (kind string, name string, parent record, is_active bool, created datetime) IN access; \
DEFINE FIELD IF NOT EXISTS company ON groups TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_company ON groups FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_kind ON groups FIELDS kind; \
DEFINE FIELD IF NOT EXISTS key_epoch ON groups TYPE int DEFAULT 1; \
DEFINE FIELD IF NOT EXISTS key_moving ON groups TYPE bool DEFAULT false; \
DEFINE TABLE IF NOT EXISTS memberships (member_role string, is_active bool, joined datetime); \
DEFINE FIELD IF NOT EXISTS company ON memberships TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS account ON memberships TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_company ON memberships FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_account ON memberships FIELDS account; \
DEFINE INDEX IF NOT EXISTS by_company_account ON memberships FIELDS company, account UNIQUE; \
DEFINE EDGE IF NOT EXISTS in_group IN access FROM accounts TO groups; \
DEFINE EDGE IF NOT EXISTS part_of IN access FROM groups TO groups; \
DEFINE EDGE IF NOT EXISTS company_group IN access FROM groups TO companies; \
DEFINE TABLE IF NOT EXISTS company_drops (state 'requested' | 'scheduled', requested_by string, requested_at int, signature string, confirmed_by string, drop_after int); \
DEFINE FIELD IF NOT EXISTS company ON company_drops TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE TABLE IF NOT EXISTS credentials (verifier string, totp string, totp_factor string, totp_last_step int, created datetime); \
DEFINE FIELD IF NOT EXISTS key_epoch ON credentials TYPE int DEFAULT 1; \
DEFINE SPACE IF NOT EXISTS enrolments; \
DEFINE TABLE IF NOT EXISTS audit_events (seq int, at int, actor string, action string, target string, reason string, line string, prev string, digest string); \
DEFINE FIELD IF NOT EXISTS company ON audit_events TYPE record REFERENCES companies ON DELETE UNSET; \
DEFINE INDEX IF NOT EXISTS by_company ON audit_events FIELDS company; \
DEFINE TABLE IF NOT EXISTS audit_heads (digest string, seq int); \
DEFINE TABLE IF NOT EXISTS identities (issuer string, subject string, linked datetime); \
DEFINE FIELD IF NOT EXISTS account ON identities TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_identity ON identities FIELDS issuer, subject UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_account ON identities FIELDS account; \
DEFINE SPACE IF NOT EXISTS oidc_pending; \
DEFINE TABLE IF NOT EXISTS passkeys (credential string, public_key string, sign_count int, backup_eligible bool, created datetime); \
DEFINE FIELD IF NOT EXISTS account ON passkeys TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_credential ON passkeys FIELDS credential UNIQUE; \
DEFINE INDEX IF NOT EXISTS by_account ON passkeys FIELDS account; \
DEFINE SPACE IF NOT EXISTS passkey_ceremonies; \
DEFINE SPACE IF NOT EXISTS sessions; \
DEFINE SPACE IF NOT EXISTS sign_ins; \
DEFINE SPACE IF NOT EXISTS throttles; \
DEFINE TABLE IF NOT EXISTS records (record_type string, revision int, key_id string, is_active bool, share_version int, rotate bool, created int, updated int); \
DEFINE FIELD IF NOT EXISTS company ON records TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS author ON records TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_author ON records FIELDS company, author; \
DEFINE TABLE IF NOT EXISTS revisions (revision int, author_key string, signature string, size int, created int); \
DEFINE FIELD IF NOT EXISTS record ON revisions TYPE record REFERENCES records ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS author ON revisions TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_record ON revisions FIELDS record; \
DEFINE FIELD IF NOT EXISTS chain_seq ON revisions TYPE int DEFAULT 0; \
DEFINE TABLE IF NOT EXISTS record_types (allowed_roles array, definition string, created int); \
DEFINE FIELD IF NOT EXISTS company ON record_types TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_company ON record_types FIELDS company; \
DEFINE TABLE IF NOT EXISTS files (size int, kept string, written int, created int); \
DEFINE FIELD IF NOT EXISTS company ON files TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS record ON files TYPE record REFERENCES records ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS author ON files TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_record ON files FIELDS record; \
DEFINE TABLE IF NOT EXISTS file_objects (offset int, object_key string, size int); \
DEFINE FIELD IF NOT EXISTS company ON file_objects TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS file ON file_objects TYPE record REFERENCES files ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_file_part ON file_objects FIELDS company, file, offset UNIQUE; \
DEFINE BUCKET IF NOT EXISTS record_files MAX 4194304; \
DEFINE TABLE IF NOT EXISTS member_keys (key_id string, agreement_public string, signing_public string, kdf_salt string, kdf_memory int, kdf_passes int, kdf_lanes int, created int); \
DEFINE FIELD IF NOT EXISTS company ON member_keys TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS account ON member_keys TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_company ON member_keys FIELDS company; \
DEFINE INDEX IF NOT EXISTS by_key ON member_keys FIELDS company, key_id UNIQUE; \
DEFINE TABLE IF NOT EXISTS chains (owner_signing string, length int, created int); \
DEFINE FIELD IF NOT EXISTS company ON chains TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE TABLE IF NOT EXISTS statements (seq int, bytes string, signature string, created int); \
DEFINE FIELD IF NOT EXISTS company ON statements TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS author ON statements TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_company ON statements FIELDS company; \
DEFINE TABLE IF NOT EXISTS group_keys (key_id string, group_public string, rotate bool, created datetime); \
DEFINE FIELD IF NOT EXISTS company ON group_keys TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS group_id ON group_keys TYPE record; \
DEFINE INDEX IF NOT EXISTS by_key ON group_keys FIELDS company, key_id UNIQUE; \
DEFINE TABLE IF NOT EXISTS shares (target_kind string, target record, access string, recipient string, created int); \
DEFINE FIELD IF NOT EXISTS company ON shares TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS record ON shares TYPE record REFERENCES records ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS by_account ON shares TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_record ON shares FIELDS record; \
DEFINE INDEX IF NOT EXISTS by_target ON shares FIELDS company, target; \
DEFINE TABLE IF NOT EXISTS recovery_keys (key_id string, public string, created datetime); \
DEFINE FIELD IF NOT EXISTS company ON recovery_keys TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS rotate ON recovery_keys TYPE bool DEFAULT false; \
DEFINE TABLE IF NOT EXISTS escrows (created int); \
DEFINE FIELD IF NOT EXISTS company ON escrows TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS account ON escrows TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS recovery_key_id ON escrows TYPE record REFERENCES recovery_keys ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_recovery_key ON escrows FIELDS company, recovery_key_id; \
DEFINE TABLE IF NOT EXISTS recovery_notices (kind string, created int); \
DEFINE FIELD IF NOT EXISTS company ON recovery_notices TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS account ON recovery_notices TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS by_account ON recovery_notices TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE INDEX IF NOT EXISTS by_member ON recovery_notices FIELDS company, account; \
DEFINE TABLE IF NOT EXISTS recovery_requests (handoff_public string, created int, expires int, redeemed int, consumed int); \
DEFINE FIELD IF NOT EXISTS company ON recovery_requests TYPE record REFERENCES companies ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS account ON recovery_requests TYPE record REFERENCES accounts ON DELETE CASCADE; \
DEFINE FIELD IF NOT EXISTS handed_by ON recovery_requests TYPE record REFERENCES accounts ON DELETE CASCADE; \
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
