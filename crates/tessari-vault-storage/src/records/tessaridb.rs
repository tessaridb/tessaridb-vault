//! Records on TessariDB: a `records` row per record, a `revisions` row per revision, and each revision's sealed body
//! in the company's keyed vault under `<record>.<revision>`, with the author's wrapped record key as its recipient.

use async_trait::async_trait;
use tessari_vault_constants::RECORD_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, KeyId, RecordId, base64url};

use super::entity::{OpenedRevision, RecordEntity};
use super::opened::opened_from;
use super::repository::{RecordRepository, Removal, Write};
use super::vault::RecordVaults;
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::record_types::role_words;
use crate::{Error, Result, answers};

/// Refuses unless `$record` is `$company`'s, the answer a record that does not exist gets; who reaches it is decided
/// before the write.
pub(crate) const COMPANYS_RECORD: &str =
    "THROW 'not found' IF (GET records:$record).company != $company;";

/// Refuses when a share was made or removed since the caller's reach was decided, so a removal committed in between
/// is never served past.
pub(crate) const SHARES_UNCHANGED: &str =
    "THROW 'stale shares' IF (GET records:$record).share_version != $share_version;";

/// Refuses unless `$author_key` is the key set `$author` keeps in `$company`, so a revision names no key but theirs.
const OWN_KEY: &str = "THROW 'unknown key' IF (GET member_keys:$own_keys).key_id != $author_key;";

/// The statements that store one revision: its row, its sealed body, and the author's wrapped key beside it.
fn revision_statements(vault: &str) -> String {
    format!(
        "CREATE revisions:$vid = {{ record: $record, revision: $revision, author: $author, author_key: $author_key, \
         signature: $signature, size: $size, created: $now }}; \
         CREATE {vault}:$vid = {{ body: $body }}; \
         ADD RECIPIENT $recipient TO {vault}:$vid KEY $wrap;"
    )
}

/// The statements adding each extra wrap as a recipient of the revision, with their bound values.
fn extra_recipients(vault: &str, extra: &[(String, Vec<u8>)]) -> (String, Vec<(String, Value)>) {
    let mut script = String::new();
    let mut parameters = Vec::new();
    for (index, (name, wrap)) in extra.iter().enumerate() {
        script.push_str(&format!(
            " ADD RECIPIENT $extra_name{index} TO {vault}:$vid KEY $extra_wrap{index};"
        ));
        parameters.push(bind(&format!("extra_name{index}"), name.as_str()));
        parameters.push(bind(&format!("extra_wrap{index}"), wrap.clone()));
    }
    (script, parameters)
}

/// The vault id of one revision of `record`.
fn revision_id(record: &str, revision: i64) -> String {
    format!("{record}.{revision}")
}

/// The recipient name the author's wrap is kept under.
fn recipient(author_key: &KeyId) -> String {
    format!("member:{}", author_key.as_str())
}

pub(crate) struct TessaridbRecordRepository {
    database: Database,
}

impl TessaridbRecordRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }

    async fn write(
        &self,
        place: &Place,
        script: &str,
        mut parameters: Vec<(String, Value)>,
        write: Write<'_>,
    ) -> Result<()> {
        let vault = RecordVaults::name(write.company);
        let sealed = write.sealed;
        let size = i64::try_from(sealed.body.len()).map_err(|_| Error::Unreadable {
            record: "record",
            reason: "a body too large to count".to_owned(),
        })?;
        parameters.extend([
            bind("company", write.company.as_str()),
            bind("record", write.record.as_str()),
            bind(
                "vid",
                revision_id(write.record.as_str(), write.revision).as_str(),
            ),
            bind("revision", write.revision),
            bind("type", write.record_type),
            bind("key_id", write.key_id.as_str()),
            bind("author", write.author.as_str()),
            bind(
                "own_keys",
                membership_id(write.company, write.author).as_str(),
            ),
            bind("author_key", sealed.author_key.as_str()),
            bind("signature", base64url::encode(&sealed.signature).as_str()),
            bind("size", size),
            bind("now", write.now),
            bind("body", base64url::encode(&sealed.body).as_str()),
            bind("recipient", recipient(&sealed.author_key).as_str()),
            bind("wrap", sealed.wrap.clone()),
        ]);
        self.database
            .transact_presenting(place, script, "", parameters, &vault, write.vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}

#[async_trait]
impl RecordRepository for TessaridbRecordRepository {
    async fn list(
        &self,
        place: &Place,
        company: &CompanyId,
        author: &AccountId,
    ) -> Result<Vec<RecordEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM records WHERE company = $company AND author = $author LIMIT $limit \
                 USING INDEX by_author;",
                vec![
                    bind("company", company.as_str()),
                    bind("author", author.as_str()),
                    bind("limit", RECORD_LIST_MAX),
                ],
            )
            .await?;
        answers::records(answers, "record")
    }

    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        record: &RecordId,
    ) -> Result<Option<RecordEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                // By address, which no index can stand in for; an address holds one record, so a ceiling of two
                // is never reached.
                "SELECT * FROM (SELECT * FROM records:$record LIMIT 2) WHERE company = $company;",
                vec![
                    bind("record", record.as_str()),
                    bind("company", company.as_str()),
                ],
            )
            .await?;
        answers::single(answers, "record")
    }

    async fn among(
        &self,
        place: &Place,
        company: &CompanyId,
        ids: Vec<String>,
    ) -> Result<Vec<RecordEntity>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM records WHERE record_id IN $ids AND company = $company LIMIT $limit;",
                vec![
                    bind(
                        "ids",
                        Value::Array(ids.into_iter().map(Value::from).collect()),
                    ),
                    bind("company", company.as_str()),
                    bind("limit", RECORD_LIST_MAX),
                ],
            )
            .await?;
        answers::records(answers, "record")
    }

    async fn create(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        write: Write<'_>,
    ) -> Result<()> {
        let revision = revision_statements(&RecordVaults::name(write.company));
        let mut guard = guard;
        // A company type narrowed after the decision refuses the write, as a changed standing does.
        let type_unchanged = match write.company_type {
            Some((id, roles)) => {
                guard.extend([
                    bind("type_id", id.as_str()),
                    ("type_roles".to_owned(), role_words(roles.words())),
                ]);
                "THROW 'standing changed' IF (GET record_types:$type_id).allowed_roles != $type_roles \
                 OR (GET record_types:$type_id).company != $company;"
            }
            None => "",
        };
        let script = format!(
            "BEGIN; {GUARD} {type_unchanged} {OWN_KEY} \
             LET $taken = GET records:$record; THROW 'record exists' IF ($taken.record_id ?? 'none') != 'none'; \
             CREATE records:$record = {{ record_id: $record, company: $company, record_type: $type, \
             revision: $revision, key_id: $key_id, author: $author, state: 'active', share_version: 0, rotate: false, created: $now, \
             updated: $now }}; {revision} {RECORD} COMMIT;"
        );
        self.write(place, &script, guard, write).await
    }

    async fn revise(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        write: Write<'_>,
        expected: i64,
    ) -> Result<()> {
        let vault = RecordVaults::name(write.company);
        let (extra, extra_values) = extra_recipients(&vault, write.extra);
        guard.push(bind("expected", expected));
        guard.push(bind("share_version", write.share_version));
        guard.extend(extra_values);
        let revision = revision_statements(&vault);
        let script = format!(
            "BEGIN; {GUARD} {COMPANYS_RECORD} {OWN_KEY} \
             THROW 'stale revision' IF (GET records:$record).revision != $expected; \
             {SHARES_UNCHANGED} \
             THROW 'rotation due' IF (GET records:$record).rotate = true AND (GET records:$record).key_id = $key_id; \
             UPDATE records:$record SET revision = $revision, key_id = $key_id, rotate = false, updated = $now; \
             {revision}{extra} {RECORD} COMMIT;"
        );
        self.write(place, &script, guard, write).await
    }

    async fn open(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        entity: &RecordEntity,
        vault_key: VaultKey,
    ) -> Result<OpenedRevision> {
        let vault = RecordVaults::name(company);
        guard.extend([
            bind("company", company.as_str()),
            bind("record", entity.id.as_str()),
            bind("vid", revision_id(&entity.id, entity.revision).as_str()),
            bind("share_version", entity.share_version),
        ]);
        // The read is on the trail before anything is opened: the transaction recording it commits first.
        let recorded =
            format!("BEGIN; {GUARD} {COMPANYS_RECORD} {SHARES_UNCHANGED} {RECORD} COMMIT;");
        let opened = format!(
            "REVEAL body FROM {vault}:$vid; INFO FOR RECIPIENTS OF {vault}:$vid; RETURN (GET revisions:$vid);"
        );
        let answers = self
            .database
            .transact_presenting(place, &recorded, &opened, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        opened_from(answers)
    }

    async fn delete(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        removal: Removal<'_>,
    ) -> Result<()> {
        super::removal::delete(&self.database, place, guard, removal).await
    }
}
