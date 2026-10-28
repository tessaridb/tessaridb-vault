//! Key sets on TessariDB: a `member_keys` row per member per company at `<company>_<account>`, and the wrapped
//! private half in the company's records vault under `keys.<account>`.

use async_trait::async_trait;
use tessari_vault_constants::KEY_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, base64url};

use super::entity::{KeyEntity, KeySet};
use super::repository::KeyRepository;
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::records::RecordVaults;
use crate::{Conflict, Error, Result, answers};

pub(crate) struct TessaridbKeyRepository {
    database: Database,
}

impl TessaridbKeyRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

fn located(company: &CompanyId, account: &AccountId) -> Vec<(String, Value)> {
    vec![
        bind("company", company.as_str()),
        bind("account", account.as_str()),
        bind("keys", membership_id(company, account).as_str()),
        bind("vid", format!("keys.{}", account.as_str()).as_str()),
    ]
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "key set",
        reason: format!("{what} did not read back as written"),
    }
}

#[async_trait]
impl KeyRepository for TessaridbKeyRepository {
    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<KeyEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM member_keys:$keys;",
                located(company, account),
            )
            .await?;
        answers::single(answers, "key set")
    }

    async fn list(&self, place: &Place, company: &CompanyId) -> Result<Vec<KeyEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM member_keys WHERE company = $company LIMIT $limit USING INDEX by_company;",
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
        set: &KeySet,
        now: i64,
        vault_key: VaultKey,
    ) -> Result<()> {
        let public = &set.public;
        guard.extend(located(company, &public.account));
        guard.extend([
            bind("key_id", public.key_id.as_str()),
            bind(
                "agreement",
                base64url::encode(&public.agreement_public).as_str(),
            ),
            bind(
                "signing",
                base64url::encode(&public.signing_public).as_str(),
            ),
            bind("salt", base64url::encode(set.kdf.salt()).as_str()),
            bind("memory", i64::from(set.kdf.memory_kib())),
            bind("passes", i64::from(set.kdf.passes())),
            bind("lanes", i64::from(set.kdf.lanes())),
            bind("wrapped", base64url::encode(set.wrapped()).as_str()),
            bind("now", now),
        ]);
        let vault = RecordVaults::name(company);
        let script = format!(
            "BEGIN; {GUARD} THROW 'keys exist' IF (GET member_keys:$keys).key_id != NONE; \
             CREATE member_keys:$keys = {{ company: $company, account: $account, key_id: $key_id, \
             agreement_public: $agreement, signing_public: $signing, kdf_salt: $salt, kdf_memory: $memory, \
             kdf_passes: $passes, kdf_lanes: $lanes, created: $now }}; \
             CREATE {vault}:$vid = {{ body: $wrapped }}; {RECORD} COMMIT;"
        );
        match self
            .database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
        {
            Err(tessari_vault_infrastructure::Error::Refused { message, .. })
                if message.starts_with("unique index by_key") =>
            {
                Err(Error::Conflict(Conflict::KeyTaken))
            }
            other => other.map(|_| ()).map_err(|error| thrown(error.into())),
        }
    }

    async fn open(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        vault_key: VaultKey,
    ) -> Result<Vec<u8>> {
        guard.extend(located(company, account));
        let vault = RecordVaults::name(company);
        // The read is on the trail before anything is opened: the transaction recording it commits first.
        let script = format!("BEGIN; {GUARD} {RECORD} COMMIT; REVEAL body FROM {vault}:$vid;");
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
                Some(Value::String(text)) => {
                    base64url::decode(&text).map_err(|_| unreadable("wrapped keys"))
                }
                _ => Err(unreadable("wrapped keys")),
            },
            _ => Err(unreadable("wrapped keys")),
        }
    }
}
