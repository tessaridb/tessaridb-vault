//! Files on TessariDB: a `files` row per file; the sealed bytes in the company's records vault under `file.<id>`, or
//! in the bucket `record_files` as one object per part at `/<company>/<file>/<part>`, each written and read whole.

use async_trait::async_trait;
use tessari_vault_constants::FILE_PART_MAX_BYTES;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value, VaultKey};
use tessari_vault_types::base64url;

use super::entity::{FileEntity, Kept};
use super::repository::{FileRef, FileRepository, Part};
use crate::access::{GUARD, bind, thrown};
use crate::audit::RECORD;
use crate::records::{COMPANYS_RECORD, RecordVaults, SHARES_UNCHANGED, parts};
use crate::{Error, Result, answers};

/// Refuses a part unless the file holds exactly the bytes before it.
const IN_ORDER: &str = "THROW 'out of order' IF (GET files:$file).written != $offset;";

pub(crate) struct TessaridbFiles {
    database: Database,
}

impl TessaridbFiles {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

fn located(at: &FileRef<'_>) -> Vec<(String, Value)> {
    vec![
        bind("company", at.company.as_str()),
        bind("record", at.record.as_str()),
        bind("file", at.file.as_str()),
        bind("author", at.author.as_str()),
        bind("share_version", at.share_version),
        bind("vid", vault_entry(at.file.as_str()).as_str()),
    ]
}

/// The bucket path of part `index` of the file.
fn part_path(at: &FileRef<'_>, index: i64) -> String {
    bucket_path(at.company.as_str(), at.file.as_str(), index)
}

/// The bucket path of part `index` of `company`'s file `file`.
pub(crate) fn bucket_path(company: &str, file: &str, index: i64) -> String {
    format!("/{company}/{file}/{index}")
}

/// The id a small file's sealed bytes are kept under in the records vault.
pub(crate) fn vault_entry(file: &str) -> String {
    format!("file.{file}")
}

pub(crate) fn part_size() -> Result<i64> {
    i64::try_from(FILE_PART_MAX_BYTES).map_err(|_| unreadable("part size"))
}

/// Which part holds the byte at `offset`.
pub(crate) fn part_index(offset: i64) -> Result<i64> {
    offset
        .checked_div(part_size()?)
        .ok_or_else(|| unreadable("part index"))
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "file",
        reason: format!("{what} did not read back as written"),
    }
}

#[async_trait]
impl FileRepository for TessaridbFiles {
    async fn get(&self, place: &Place, at: &FileRef<'_>) -> Result<Option<FileEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                // By address, as records are read.
                "SELECT * FROM (SELECT * FROM files:$file LIMIT 2) WHERE record = $record AND company = $company;",
                located(at),
            )
            .await?;
        answers::single(answers, "file")
    }

    async fn create(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        size: i64,
        kept: Kept,
        now: i64,
    ) -> Result<()> {
        guard.extend(located(at));
        guard.extend([
            bind("size", size),
            bind("kept", kept.word()),
            bind("now", now),
        ]);
        let script = format!(
            "BEGIN; {GUARD} {COMPANYS_RECORD} {SHARES_UNCHANGED} \
             CREATE files:$file = {{ file_id: $file, company: $company, record: $record, author: $author, \
             size: $size, kept: $kept, written: 0, created: $now }}; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", guard)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn delete(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        file: &FileEntity,
        vault_key: VaultKey,
    ) -> Result<()> {
        let vault = RecordVaults::name(at.company);
        guard.extend(located(at));
        let mut bytes = String::new();
        if file.kept == Kept::Vault.word() {
            bytes.push_str(&format!("DELETE {vault}:$vid;"));
        } else {
            for index in 0..parts(file.size)? {
                bytes.push_str(&format!(" DELETE record_files:$part{index};"));
                guard.push(bind(&format!("part{index}"), part_path(at, index).as_str()));
            }
        }
        // The uploader alone, checked again where the row is deleted, so a file named by nobody else is never another's.
        let script = format!(
            "BEGIN; {GUARD} THROW 'not found' IF (GET files:$file).author != $author \
             OR (GET files:$file).record != $record; {bytes} DELETE files:$file; {RECORD} COMMIT;"
        );
        self.database
            .transact_presenting(place, &script, "", guard, &vault, vault_key)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn write(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        part: Part<'_>,
        vault_key: VaultKey,
    ) -> Result<()> {
        guard.extend(located(at));
        guard.extend([bind("offset", part.offset), bind("end", part.end)]);
        let record = match part.completes {
            Some(audit) => {
                guard.extend(audit);
                RECORD
            }
            None => "",
        };
        let vault = RecordVaults::name(at.company);
        let store = match part.kept {
            Kept::Vault => {
                guard.push(bind("body", base64url::encode(part.bytes).as_str()));
                format!("CREATE {vault}:$vid = {{ body: $body }};")
            }
            Kept::Bucket => {
                let path = part_path(at, part_index(part.offset)?);
                guard.extend([
                    bind("part", path.as_str()),
                    bind("bytes", part.bytes.to_vec()),
                ]);
                "PUT record_files:$part = $bytes;".to_owned()
            }
            Kept::S3 => return Err(unreadable("backend")),
        };
        let script = format!(
            "BEGIN; {GUARD} {SHARES_UNCHANGED} {IN_ORDER} {store} UPDATE files:$file SET written = $end; {record} \
             COMMIT;"
        );
        let written = match part.kept {
            Kept::Vault => {
                self.database
                    .transact_presenting(place, &script, "", guard, &vault, vault_key)
                    .await
            }
            Kept::Bucket => self.database.transact_at(place, &script, "", guard).await,
            Kept::S3 => return Err(unreadable("backend")),
        };
        written.map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn read(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        kept: Kept,
        (offset, limit): (i64, i64),
        vault_key: VaultKey,
    ) -> Result<Vec<u8>> {
        guard.extend(located(at));
        // The read is on the trail before anything is read: the transaction recording it commits first.
        let recorded = format!("BEGIN; {GUARD} {SHARES_UNCHANGED} {RECORD} COMMIT;");
        match kept {
            Kept::Vault => {
                let vault = RecordVaults::name(at.company);
                let opened = format!("REVEAL body FROM {vault}:$vid;");
                let answers = self
                    .database
                    .transact_presenting(place, &recorded, &opened, guard, &vault, vault_key)
                    .await
                    .map_err(|error| thrown(error.into()))?;
                let whole = revealed(answers)?;
                Ok(slice(&whole, offset, limit))
            }
            Kept::Bucket => {
                // Only the parts the range touches are read, each whole, and the range is cut from them here.
                let first = part_index(offset)?;
                let last = part_index(offset.saturating_add(limit).saturating_sub(1))?;
                let mut reads = String::new();
                for index in first..=last {
                    guard.push(bind(&format!("p{index}"), part_path(at, index).as_str()));
                    reads.push_str(&format!(" READ record_files:$p{index};"));
                }
                let answers = self
                    .database
                    .transact_at(place, &recorded, &reads, guard)
                    .await
                    .map_err(|error| thrown(error.into()))?;
                let parts = usize::try_from(last.saturating_sub(first))
                    .ok()
                    .and_then(|count| count.checked_add(1))
                    .ok_or_else(|| unreadable("part count"))?;
                let skipped = first
                    .checked_mul(part_size()?)
                    .and_then(|start| offset.checked_sub(start))
                    .ok_or_else(|| unreadable("part offset"))?;
                Ok(slice(&joined(answers, parts)?, skipped, limit))
            }
            Kept::S3 => Err(unreadable("backend")),
        }
    }
}

/// The revealed body from the last answer, as `{ body: <base64url> }`.
fn revealed(answers: Vec<Answer>) -> Result<Vec<u8>> {
    match answers.into_iter().last() {
        Some(Answer::Value {
            value: Value::Object(mut fields),
            ..
        }) => match fields.remove("body") {
            Some(Value::String(text)) => base64url::decode(&text).map_err(|_| unreadable("body")),
            _ => Err(unreadable("body")),
        },
        _ => Err(unreadable("body")),
    }
}

/// The bytes of the last `parts` answers in order; a part past the file's end reads as nothing and ends it.
fn joined(answers: Vec<Answer>, parts: usize) -> Result<Vec<u8>> {
    let skip = answers
        .len()
        .checked_sub(parts)
        .ok_or_else(|| unreadable("parts"))?;
    let mut bytes = Vec::new();
    for answer in answers.into_iter().skip(skip) {
        match answer {
            Answer::Value {
                value: Value::Bytes(part),
                ..
            } => bytes.extend_from_slice(&part),
            Answer::Value {
                value: Value::None, ..
            } => break,
            _ => return Err(unreadable("part")),
        }
    }
    Ok(bytes)
}

/// At most `limit` bytes of `whole` from `offset`.
fn slice(whole: &[u8], offset: i64, limit: i64) -> Vec<u8> {
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(whole.len());
    let length = usize::try_from(limit).unwrap_or(0);
    let end = start.saturating_add(length).min(whole.len());
    whole
        .get(start..end)
        .map(<[u8]>::to_vec)
        .unwrap_or_default()
}
