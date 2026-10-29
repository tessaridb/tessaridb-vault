//! Group keys on TessariDB: a `group_keys` row per keyed group at the group's id, and a vault record `group.<group>`
//! in the company's records vault whose recipients `member:<key id>` hold the wraps.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, GroupId, base64url};

use super::entity::{GroupKeyEntity, MemberWrap, NewGroupKey};
use super::repository::GroupKeyRepository;
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::keys::KeyEntity;
use crate::records::RecordVaults;
use crate::{Error, Result, answers};

/// The recipient prefix a member's wrap is kept under.
const MEMBER: &str = "member:";

pub(crate) struct TessaridbGroupKeyRepository {
    database: Database,
}

impl TessaridbGroupKeyRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

fn vid(group: &GroupId) -> String {
    format!("group.{}", group.as_str())
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "group key",
        reason: format!("{what} did not read back as written"),
    }
}

/// The statements adding each wrap as a recipient `member:<key id>`, with their bound values.
fn recipients(vault: &str, wraps: &[MemberWrap]) -> (String, Vec<(String, Value)>) {
    let mut script = String::new();
    let mut parameters = Vec::new();
    for (index, wrap) in wraps.iter().enumerate() {
        script.push_str(&format!(
            " ADD RECIPIENT $name{index} TO {vault}:$vid KEY $wrap{index};"
        ));
        parameters.push(bind(
            &format!("name{index}"),
            format!("{MEMBER}{}", wrap.key_id().as_str()).as_str(),
        ));
        parameters.push(bind(&format!("wrap{index}"), wrap.wrap().to_vec()));
    }
    (script, parameters)
}

#[async_trait]
impl GroupKeyRepository for TessaridbGroupKeyRepository {
    async fn reach(
        &self,
        place: &Place,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Option<Vec<KeyEntity>>> {
        super::reach::reach(&self.database, place, company, group).await
    }

    async fn get(&self, place: &Place, group: &GroupId) -> Result<Option<GroupKeyEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM group_keys:$group;",
                vec![bind("group", group.as_str())],
            )
            .await?;
        answers::single(answers, "group key")
    }

    async fn wraps(
        &self,
        place: &Place,
        audit: Option<Vec<(String, Value)>>,
        company: &CompanyId,
        group: &GroupId,
        vault_key: VaultKey,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let vault = RecordVaults::name(company);
        let (recorded, mut parameters) = match audit {
            Some(guard) => (format!("BEGIN; {GUARD} {RECORD} COMMIT;"), guard),
            None => (String::new(), Vec::new()),
        };
        parameters.push(bind("vid", vid(group).as_str()));
        let script = format!("{recorded} INFO FOR RECIPIENTS OF {vault}:$vid;");
        let answers = self
            .database
            .run_presenting(place, &script, parameters, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        let Some(Answer::Value {
            value: Value::Object(mut answer),
            ..
        }) = answers.into_iter().last()
        else {
            return Err(unreadable("recipients"));
        };
        let Some(Value::Object(named)) = answer.remove("recipients") else {
            return Err(unreadable("recipients"));
        };
        named
            .into_iter()
            .filter_map(|(name, material)| {
                name.strip_prefix(MEMBER).map(|key| match material {
                    Value::Bytes(wrap) => Ok((key.to_owned(), wrap)),
                    _ => Err(unreadable("a wrap")),
                })
            })
            .collect()
    }

    async fn keep(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        key: &NewGroupKey,
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(company);
        let (added, wraps) = recipients(&vault, key.wraps());
        guard.extend(wraps);
        guard.extend([
            bind("company", company.as_str()),
            bind("group", group.as_str()),
            bind("vid", vid(group).as_str()),
            bind("key_id", key.key_id().as_str()),
            bind("public", base64url::encode(key.public()).as_str()),
        ]);
        let script = format!(
            "BEGIN; {GUARD} THROW 'keys exist' IF (GET group_keys:$group).key_id != NONE; \
             CREATE group_keys:$group = {{ company: $company, group_id: $group, key_id: $key_id, \
             group_public: $public, rotate: false, created: time::now() }}; \
             CREATE {vault}:$vid = {{ body: $public }};{added} {RECORD} COMMIT;"
        );
        self.database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn rekey(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        key: &NewGroupKey,
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(company);
        let (added, wraps) = recipients(&vault, key.wraps());
        guard.extend(wraps);
        guard.extend([
            bind("company", company.as_str()),
            bind("group", group.as_str()),
            bind("vid", vid(group).as_str()),
            bind("key_id", key.key_id().as_str()),
            bind("public", base64url::encode(key.public()).as_str()),
            bind("named", format!("group:{}", key.key_id().as_str()).as_str()),
        ]);
        // The vault record is made again, so no wrap of the old key survives on it.
        let script = format!(
            "BEGIN; {GUARD} THROW 'keys exist' IF (GET group_keys:$group).rotate != true \
             OR (GET group_keys:$group).company != $company; \
             UPDATE group_keys:$group SET key_id = $key_id, group_public = $public, rotate = false; \
             DELETE {vault}:$vid; CREATE {vault}:$vid = {{ body: $public }};{added} \
             UPDATE shares SET recipient = $named WHERE company = $company AND target = $group; {RECORD} COMMIT;"
        );
        self.database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn add_wraps(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        wraps: &[MemberWrap],
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(company);
        let (added, parameters) = recipients(&vault, wraps);
        guard.extend(parameters);
        guard.push(bind("vid", vid(group).as_str()));
        let script = format!("BEGIN; {GUARD}{added} {RECORD} COMMIT;");
        self.database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
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
