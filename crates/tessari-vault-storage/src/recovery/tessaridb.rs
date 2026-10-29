//! Recovery on TessariDB: a `recovery_keys` row at the company's id, and a vault record `recovery` in the company's
//! records vault whose recipients `owner:<key id>` hold the owners' wraps.

use async_trait::async_trait;
use tessari_vault_constants::KEY_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, base64url};

use super::entity::RecoveryKeyEntity;
use super::repository::RecoveryRepository;
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::group_keys::{NewGroupKey, recipient_wraps, recipients};
use crate::keys::KeyEntity;
use crate::records::RecordVaults;
use crate::{Result, answers};

/// The recipient prefix an owner's wrap is kept under.
const OWNER: &str = "owner:";

/// The recovery key's record in the company's vault.
const VID: &str = "recovery";

pub(crate) struct TessaridbRecoveryRepository {
    database: Database,
}

impl TessaridbRecoveryRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl RecoveryRepository for TessaridbRecoveryRepository {
    async fn get(&self, place: &Place, company: &CompanyId) -> Result<Option<RecoveryKeyEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM recovery_keys:$company;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        answers::single(answers, "recovery key")
    }

    async fn owners(&self, place: &Place, company: &CompanyId) -> Result<Vec<KeyEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM member_keys WHERE company = $company AND account IN \
                 (SELECT account FROM memberships WHERE company = $company AND member_role = 'owner' \
                 AND state = 'active' LIMIT $limit) LIMIT $limit;",
                vec![
                    bind("company", company.as_str()),
                    bind("limit", KEY_LIST_MAX),
                ],
            )
            .await?;
        answers::records(answers, "key set")
    }

    async fn keep(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        key: &NewGroupKey,
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(company);
        let (added, wraps) = recipients(&vault, OWNER, key.wraps());
        guard.extend(wraps);
        guard.extend([
            bind("company", company.as_str()),
            bind("vid", VID),
            bind("key_id", key.key_id().as_str()),
            bind("public", base64url::encode(key.public()).as_str()),
        ]);
        let script = format!(
            "BEGIN; {GUARD} THROW 'recovery keyed' IF (GET recovery_keys:$company).key_id != NONE; \
             CREATE recovery_keys:$company = {{ company: $company, key_id: $key_id, public: $public, \
             created: time::now() }}; CREATE {vault}:$vid = {{ body: $public }};{added} {RECORD} COMMIT;"
        );
        self.database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn wraps(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        vault_key: VaultKey,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let vault = RecordVaults::name(company);
        guard.push(bind("vid", VID));
        let script =
            format!("BEGIN; {GUARD} {RECORD} COMMIT; INFO FOR RECIPIENTS OF {vault}:$vid;");
        let answers = self
            .database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        recipient_wraps(answers, OWNER, "recovery key")
    }

    async fn own_key(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM member_keys:$keys;",
                vec![bind("keys", membership_id(company, account).as_str())],
            )
            .await?;
        let key: Option<KeyEntity> = answers::single(answers, "key set")?;
        Ok(key.map(|key| key.key_id))
    }
}
