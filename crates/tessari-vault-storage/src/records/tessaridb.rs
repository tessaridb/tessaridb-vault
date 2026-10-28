//! Records on TessariDB: a `records` row per record, a `revisions` row per revision, and each revision's sealed body
//! in the company's keyed vault under `<record>.<revision>`, with the author's wrapped record key as its recipient.

use std::collections::BTreeMap;

use async_trait::async_trait;
use tessari_vault_constants::RECORD_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, KeyId, RecordId, base64url};

use super::entity::{RecordEntity, Sealed};
use super::repository::{RecordRepository, Write};
use super::vault::RecordVaults;
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::record_types::role_words;
use crate::{Error, Result, answers};

/// Refuses unless `$record` is `$company`'s and `$author` wrote it, the answer a record that does not exist gets.
pub(crate) const AUTHORS_RECORD: &str = "\
THROW 'not found' IF (GET records:$record).company != $company OR (GET records:$record).author != $author;";

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
            .run_presenting(place, script, parameters, &vault, write.vault_key)
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
            .run_at(
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
        author: &AccountId,
        record: &RecordId,
    ) -> Result<Option<RecordEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM records WHERE record_id = $record AND company = $company AND author = $author \
                 USING INDEX by_id;",
                vec![
                    bind("record", record.as_str()),
                    bind("company", company.as_str()),
                    bind("author", author.as_str()),
                ],
            )
            .await?;
        answers::single(answers, "record")
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
             CREATE records:$record = {{ record_id: $record, company: $company, record_type: $type, \
             revision: $revision, key_id: $key_id, author: $author, state: 'active', created: $now, \
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
        guard.push(bind("expected", expected));
        let revision = revision_statements(&RecordVaults::name(write.company));
        let script = format!(
            "BEGIN; {GUARD} {AUTHORS_RECORD} {OWN_KEY} \
             THROW 'stale revision' IF (GET records:$record).revision != $expected; \
             UPDATE records:$record SET revision = $revision, key_id = $key_id, updated = $now; \
             {revision} {RECORD} COMMIT;"
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
    ) -> Result<Sealed> {
        let vault = RecordVaults::name(company);
        guard.extend([
            bind("company", company.as_str()),
            bind("record", entity.id.as_str()),
            bind("author", entity.author.as_str()),
            bind("vid", revision_id(&entity.id, entity.revision).as_str()),
        ]);
        // The read is on the trail before anything is opened: the transaction recording it commits first.
        let script = format!(
            "BEGIN; {GUARD} {AUTHORS_RECORD} {RECORD} COMMIT; \
             REVEAL body FROM {vault}:$vid; \
             INFO FOR RECIPIENTS OF {vault}:$vid; \
             RETURN (GET revisions:$vid);"
        );
        let answers = self
            .database
            .run_presenting(place, &script, guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        sealed_from(answers)
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "revision",
        reason: format!("{what} did not read back as written"),
    }
}

fn object(answer: Option<Answer>, what: &str) -> Result<BTreeMap<String, Value>> {
    match answer {
        Some(Answer::Value {
            value: Value::Object(fields),
            ..
        }) => Ok(fields),
        _ => Err(unreadable(what)),
    }
}

fn text<'a>(fields: &'a BTreeMap<String, Value>, field: &str) -> Result<&'a str> {
    match fields.get(field) {
        Some(Value::String(text)) => Ok(text),
        _ => Err(unreadable(field)),
    }
}

/// The sealed parts from the last three answers: the revealed body, the recipient set, and the revision row.
fn sealed_from(mut answers: Vec<Answer>) -> Result<Sealed> {
    let row = object(answers.pop(), "revision row")?;
    let recipients = answers.pop();
    let revealed = object(answers.pop(), "revealed body")?;
    let author_key =
        KeyId::parse(text(&row, "author_key")?).map_err(|_| unreadable("author key"))?;
    let named = recipient(&author_key);
    let wrap = wrap_named(recipients, &named)?;
    let body = base64url::decode(text(&revealed, "body")?).map_err(|_| unreadable("body"))?;
    let signature =
        base64url::decode(text(&row, "signature")?).map_err(|_| unreadable("signature"))?;
    Sealed::new(body, signature, author_key, wrap).map_err(|_| unreadable("sealed parts"))
}

/// The material kept under `name` in a recipient set, which answers as `{ recipients: { <name>: <bytes> } }`.
fn wrap_named(recipients: Option<Answer>, name: &str) -> Result<Vec<u8>> {
    let mut answer = object(recipients, "recipients")?;
    let Some(Value::Object(mut named)) = answer.remove("recipients") else {
        return Err(unreadable("recipients"));
    };
    match named.remove(name) {
        Some(Value::Bytes(material)) => Ok(material),
        _ => Err(unreadable("the author's wrapped key")),
    }
}
