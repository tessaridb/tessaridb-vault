//! Shares on TessariDB: a `shares` row per record and target, `<record>_<target>`, kept beside the graph because an
//! edge cannot be indexed, and the share's wrap as a recipient of the record's latest revision in the company's vault.

use async_trait::async_trait;
use tessari_vault_constants::RECORD_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::{AccountId, CompanyId};

use super::entity::{DirectGroup, Named, ShareEntity, ShareTarget};
use super::repository::{ShareRemoval, ShareRepository, ShareWrite};
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::records::{RecordVaults, SHARES_UNCHANGED};
use crate::{Result, answers};

/// Refuses a target outside the company and a target holding no key, or another key than the one wrapped to.
const GROUP_KEY: &str = "\
THROW 'not found' IF $target != $company AND (GET groups:$target).company != companies:$company; \
THROW 'not found' IF $target = $company AND (GET companies:$target) = NONE; \
THROW 'no key' IF (GET group_keys:$target).key_id = NONE; \
THROW 'unknown key' IF (GET group_keys:$target).key_id != $key_id;";

/// The same for a member, whose membership and key set share the row id `<company>_<account>`.
const MEMBER_KEY: &str = "\
THROW 'not found' IF (GET memberships:$membership).company != companies:$company \
OR (GET memberships:$membership).is_active != true; \
THROW 'no key' IF (GET member_keys:$membership).key_id = NONE; \
THROW 'unknown key' IF (GET member_keys:$membership).key_id != $key_id;";

/// The statements retiring the keys of `retired` groups and every record shared with any of `retired`, or written
/// by `author`: each is marked for rotation, and a record's share version moves so a read decided before the commit
/// refuses. Binds `$retired`, `$retired_author` and `$retired_limit` beside the caller's `$company`.
pub(crate) async fn retiring(
    database: &Database,
    place: &Place,
    parameters: &mut Vec<(String, Value)>,
    company: &CompanyId,
    retired: Vec<String>,
    author: Option<&AccountId>,
) -> Result<String> {
    let targets = retired
        .iter()
        .enumerate()
        .map(|(index, group)| {
            if group == company.as_str() {
                format!("companies:$r{index}")
            } else {
                format!("groups:$r{index}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    for (index, group) in retired.iter().enumerate() {
        parameters.push(bind(&format!("r{index}"), group.as_str()));
    }
    parameters.push(bind("retired_limit", RECORD_LIST_MAX));
    let affected: Vec<Named> = answers::records(
        database
            .read_at(
                place,
                &format!(
                    "SELECT record AS name FROM shares WHERE company = companies:$company \
                     AND target IN [{targets}] LIMIT $retired_limit;"
                ),
                parameters.clone(),
            )
            .await?,
        "share",
    )?;
    let mut statements = format!(
        "UPDATE group_keys SET rotate = true WHERE company = companies:$company AND group_id IN [{targets}];"
    );
    for (index, record) in affected.iter().enumerate() {
        statements.push_str(&format!(
            " UPDATE records:$affected{index} SET rotate = true, share_version = share_version + 1;"
        ));
        parameters.push(bind(&format!("affected{index}"), record.name.as_str()));
    }
    if let Some(author) = author {
        parameters.push(bind("retired_author", author.as_str()));
        statements.push_str(
            " UPDATE records SET rotate = true, share_version = share_version + 1 \
             WHERE company = companies:$company AND author = accounts:$retired_author;",
        );
    }
    Ok(statements)
}

pub(crate) struct TessaridbShareRepository {
    database: Database,
}

impl TessaridbShareRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl ShareRepository for TessaridbShareRepository {
    async fn direct_groups(&self, place: &Place, account: &AccountId) -> Result<Vec<DirectGroup>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT id, company, parent FROM accounts:$account->in_group->groups;",
                vec![bind("account", account.as_str())],
            )
            .await?;
        answers::records(answers, "group")
    }

    async fn of_record(&self, place: &Place, record: &str) -> Result<Vec<ShareEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT target, access, recipient FROM shares WHERE record = records:$record LIMIT $limit \
                 USING INDEX by_record;",
                vec![bind("record", record), bind("limit", RECORD_LIST_MAX)],
            )
            .await?;
        answers::records(answers, "share")
    }

    async fn shared_with(
        &self,
        place: &Place,
        company: &CompanyId,
        target: &str,
    ) -> Result<Vec<String>> {
        let table = if target == company.as_str() {
            "companies"
        } else {
            "groups"
        };
        let rows: Vec<Named> = answers::records(
            self.database
                .read_at(
                    place,
                    &format!(
                        "SELECT record AS name FROM shares WHERE company = companies:$company \
                         AND target = {table}:$target LIMIT $limit USING INDEX by_target;"
                    ),
                    vec![
                        bind("company", company.as_str()),
                        bind("target", target),
                        bind("limit", RECORD_LIST_MAX),
                    ],
                )
                .await?,
            "share",
        )?;
        Ok(rows.into_iter().map(|row| row.name).collect())
    }

    async fn member_key(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>> {
        let rows: Vec<Named> = answers::records(
            self.database
                .read_at(
                    place,
                    "SELECT key_id AS name FROM member_keys:$keys;",
                    vec![bind("keys", membership_id(company, account).as_str())],
                )
                .await?,
            "key set",
        )?;
        Ok(rows.into_iter().next().map(|row| row.name))
    }

    async fn create(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        write: ShareWrite<'_>,
    ) -> Result<()> {
        let vault = RecordVaults::name(write.company);
        let share = write.share;
        let target = share.target.id();
        let next = write
            .share_version
            .checked_add(1)
            .ok_or_else(|| crate::Error::Unreadable {
                record: "record",
                reason: "a share count past its range".to_owned(),
            })?;
        let target_key = match &share.target {
            ShareTarget::Group(_) => GROUP_KEY,
            ShareTarget::Member(account) => {
                guard.push(bind(
                    "membership",
                    membership_id(write.company, account).as_str(),
                ));
                MEMBER_KEY
            }
        };
        let target_ref = if target == write.company.as_str() {
            "companies:$target"
        } else {
            "groups:$target"
        };
        guard.extend([
            bind("company", write.company.as_str()),
            bind("record", write.record.as_str()),
            bind("revision", write.revision),
            bind("share_version", write.share_version),
            bind("next", next),
            bind(
                "vid",
                format!("{}.{}", write.record.as_str(), write.revision).as_str(),
            ),
            bind(
                "sid",
                format!("{}_{target}", write.record.as_str()).as_str(),
            ),
            bind("kind", share.target.kind()),
            bind("target", target),
            bind("access", share.access.word()),
            bind("key_id", share.key_id.as_str()),
            bind("recipient", share.recipient().as_str()),
            bind("wrap", share.wrap.clone()),
            bind("by", write.by.as_str()),
            bind("now", write.now),
        ]);
        let script = format!(
            "BEGIN; {GUARD} \
             THROW 'not found' IF (GET records:$record).company != companies:$company; \
             THROW 'stale revision' IF (GET records:$record).revision != $revision; \
             {SHARES_UNCHANGED} \
             {target_key} \
             THROW 'already shared' IF (GET shares:$sid).record != NONE; \
             CREATE shares:$sid = {{ company: companies:$company, record: records:$record, target_kind: $kind, target: {target_ref}, \
             access: $access, recipient: $recipient, by_account: accounts:$by, created: $now }}; \
             UPDATE records:$record SET share_version = $next; \
             ADD RECIPIENT $recipient TO {vault}:$vid KEY $wrap; {RECORD} COMMIT;"
        );
        self.database
            .transact_presenting(place, &script, "", guard, &vault, write.vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn remove(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        removal: ShareRemoval<'_>,
    ) -> Result<()> {
        let vault = RecordVaults::name(removal.company);
        let next =
            removal
                .share_version
                .checked_add(1)
                .ok_or_else(|| crate::Error::Unreadable {
                    record: "record",
                    reason: "a share version past its range".to_owned(),
                })?;
        guard.extend([
            bind("company", removal.company.as_str()),
            bind("record", removal.record.as_str()),
            bind("revision", removal.revision),
            bind("share_version", removal.share_version),
            bind("next", next),
            bind(
                "vid",
                format!("{}.{}", removal.record.as_str(), removal.revision).as_str(),
            ),
            bind(
                "sid",
                format!("{}_{}", removal.record.as_str(), removal.target).as_str(),
            ),
        ]);
        // Older revisions keep their wraps: the removed member could already open them. The next revision rotates.
        let script = format!(
            "BEGIN; {GUARD} \
             THROW 'not found' IF (GET records:$record).company != companies:$company; \
             THROW 'stale revision' IF (GET records:$record).revision != $revision; \
             {SHARES_UNCHANGED} \
             THROW 'not found' IF (GET shares:$sid).record != records:$record; \
             REMOVE RECIPIENT (GET shares:$sid).recipient FROM {vault}:$vid; \
             DELETE shares:$sid; \
             UPDATE records:$record SET share_version = $next, rotate = true; {RECORD} COMMIT;"
        );
        self.database
            .transact_presenting(place, &script, "", guard, &vault, removal.vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}
