//! Recovery requests on TessariDB: a `recovery_requests` row at the request's id, and the owner's handoff wrap in the
//! company's records vault at `handoff.<request>`. Every act reads the row inside its own transaction, so a request
//! is handed over once, redeemed once and used once however many callers race for it.

use async_trait::async_trait;
use tessari_vault_constants::RECOVERY_REQUEST_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, KeyId, base64url};

use super::entity::RequestEntity;
use super::repository::{Asking, RecoveryRequestRepository, Rewrapped};
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::records::RecordVaults;
use crate::{Error, Result, answers};

pub(crate) struct TessaridbRecoveryRequestRepository {
    database: Database,
}

impl TessaridbRecoveryRequestRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

fn located(company: &CompanyId, id: &KeyId) -> Vec<(String, Value)> {
    vec![
        bind("company", company.as_str()),
        bind("id", id.as_str()),
        bind("hid", format!("handoff.{}", id.as_str()).as_str()),
    ]
}

/// Refuses unless request `$id` is the company's and, with `$account` bound, the caller's.
const THEIRS: &str = "THROW 'not found' IF (GET recovery_requests:$id).company != $company \
OR (GET recovery_requests:$id).account != $account;";

#[async_trait]
impl RecoveryRequestRepository for TessaridbRecoveryRequestRepository {
    async fn open(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        id: &KeyId,
        asking: Asking<'_>,
    ) -> Result<()> {
        guard.extend(located(company, id));
        guard.extend([
            bind("account", asking.account.as_str()),
            bind("keys", membership_id(company, asking.account).as_str()),
            bind("handoff", base64url::encode(asking.handoff_public).as_str()),
            bind("expires", asking.expires),
        ]);
        let script = format!(
            "BEGIN; {GUARD} THROW 'no escrow' IF (GET escrows:$keys).recovery_key_id = NONE; \
             CREATE recovery_requests:$id = {{ request_id: $id, company: $company, account: $account, \
             handoff_public: $handoff, created: $audit_at, expires: $expires }}; {RECORD} COMMIT;"
        );
        self.database
            .run_at(place, &script, guard)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn open_requests(
        &self,
        place: &Place,
        company: &CompanyId,
        now: i64,
    ) -> Result<Vec<RequestEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM recovery_requests WHERE company = $company AND handed_by = NONE \
                 AND expires > $now LIMIT $limit USING INDEX by_company;",
                vec![
                    bind("company", company.as_str()),
                    bind("now", now),
                    bind("limit", RECOVERY_REQUEST_LIST_MAX),
                ],
            )
            .await?;
        answers::records(answers, "recovery request")
    }

    async fn hand_over(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        id: &KeyId,
        wrap: &[u8],
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(company);
        guard.extend(located(company, id));
        guard.push(bind("wrap", base64url::encode(wrap).as_str()));
        let script = format!(
            "BEGIN; {GUARD} THROW 'not found' IF (GET recovery_requests:$id).company != $company; \
             THROW 'recovery closed' IF (GET recovery_requests:$id).handed_by != NONE \
             OR (GET recovery_requests:$id).expires <= $audit_at; \
             LET $member = (GET recovery_requests:$id).account; \
             UPDATE recovery_requests:$id SET handed_by = $audit_actor; \
             CREATE {vault}:$hid = {{ body: $wrap }}; \
             CREATE recovery_notices = {{ company: $company, account: $member, by_account: $audit_actor, \
             kind: 'recovery', created: $audit_at }}; {RECORD} COMMIT;"
        );
        self.database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn redeem(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        id: &KeyId,
        vault_key: VaultKey,
    ) -> Result<Vec<u8>> {
        let vault = RecordVaults::name(company);
        guard.extend(located(company, id));
        guard.push(bind("account", account.as_str()));
        // Redeemed is written in the transaction that read it unredeemed: a racing second redemption conflicts.
        let script = format!(
            "BEGIN; {GUARD} {THEIRS} \
             THROW 'not handed over' IF (GET recovery_requests:$id).handed_by = NONE; \
             THROW 'recovery closed' IF (GET recovery_requests:$id).redeemed != NONE \
             OR (GET recovery_requests:$id).expires <= $audit_at; \
             UPDATE recovery_requests:$id SET redeemed = $audit_at; {RECORD} COMMIT; \
             REVEAL body FROM {vault}:$hid;"
        );
        let answers = self
            .database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        match answers.into_iter().last() {
            Some(Answer::Value {
                value: Value::Object(mut fields),
                ..
            }) => match fields.remove("body") {
                Some(Value::String(text)) => base64url::decode(&text).map_err(|_| unreadable()),
                _ => Err(unreadable()),
            },
            _ => Err(unreadable()),
        }
    }

    async fn record(&self, place: &Place, guard: Vec<(String, Value)>) -> Result<()> {
        let script = format!("BEGIN; {GUARD} {RECORD} COMMIT;");
        self.database
            .run_at(place, &script, guard)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn rewrap(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        keys: Rewrapped<'_>,
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(company);
        guard.extend(located(company, keys.request));
        guard.extend([
            bind("account", account.as_str()),
            bind("keys", membership_id(company, account).as_str()),
            bind("kid", format!("keys.{}", account.as_str()).as_str()),
            bind("salt", base64url::encode(keys.kdf.salt()).as_str()),
            bind("memory", i64::from(keys.kdf.memory_kib())),
            bind("passes", i64::from(keys.kdf.passes())),
            bind("lanes", i64::from(keys.kdf.lanes())),
            bind("wrapped", base64url::encode(keys.wrapped).as_str()),
        ]);
        let script = format!(
            "BEGIN; {GUARD} {THEIRS} \
             THROW 'recovery closed' IF (GET recovery_requests:$id).redeemed = NONE \
             OR (GET recovery_requests:$id).consumed != NONE; \
             UPDATE recovery_requests:$id SET consumed = $audit_at; \
             UPDATE member_keys:$keys SET kdf_salt = $salt, kdf_memory = $memory, kdf_passes = $passes, \
             kdf_lanes = $lanes; DELETE {vault}:$kid; CREATE {vault}:$kid = {{ body: $wrapped }}; \
             {RECORD} COMMIT;"
        );
        self.database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}

fn unreadable() -> Error {
    Error::Unreadable {
        record: "recovery request",
        reason: "the handoff did not read back as written".to_owned(),
    }
}
