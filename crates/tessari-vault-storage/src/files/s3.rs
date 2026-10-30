//! Large encrypted files on TessariDB S3. An upload is visible only after its unique object key and the file's new
//! written length commit together; an unfinished attempt is unreachable ciphertext, never a published file part.

use async_trait::async_trait;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::s3::ObjectStorage;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value, VaultKey};
use tessari_vault_types::base64url;

use super::entity::{FileEntity, FileObject, Kept};
use super::repository::{FileRef, FileRepository, Part};
use super::tessaridb::{TessaridbFiles, part_index};
use crate::access::{GUARD, bind, thrown};
use crate::answers::records;
use crate::audit::RECORD;
use crate::records::SHARES_UNCHANGED;
use crate::{Error, Result};

const IN_ORDER: &str = "THROW 'out of order' IF (GET files:$file).written != $offset;";

pub(crate) struct S3Files {
    inner: TessaridbFiles,
    objects: std::sync::Arc<dyn ObjectStorage>,
    database: Database,
}

impl S3Files {
    pub(crate) fn new(database: Database, objects: std::sync::Arc<dyn ObjectStorage>) -> Self {
        Self {
            inner: TessaridbFiles::new(database.clone()),
            objects,
            database,
        }
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "file object",
        reason: format!("{what} did not read back as written"),
    }
}

fn located(at: &FileRef<'_>) -> Vec<(String, Value)> {
    vec![
        bind("company", at.company.as_str()),
        bind("record", at.record.as_str()),
        bind("file", at.file.as_str()),
        bind("author", at.author.as_str()),
        bind("share_version", at.share_version),
    ]
}

fn object_key(at: &FileRef<'_>, offset: i64) -> Result<String> {
    let attempt = base64url::encode(&random::bytes::<16>()?);
    Ok(format!(
        "{}/{}/{offset}/{attempt}",
        at.company.as_str(),
        at.file.as_str()
    ))
}

#[async_trait]
impl FileRepository for S3Files {
    async fn get(&self, place: &Place, at: &FileRef<'_>) -> Result<Option<FileEntity>> {
        self.inner.get(place, at).await
    }

    async fn create(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        size: i64,
        kept: Kept,
        now: i64,
    ) -> Result<()> {
        self.inner.create(place, guard, at, size, kept, now).await
    }

    async fn write(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        part: Part<'_>,
        vault_key: VaultKey,
    ) -> Result<()> {
        if part.kept != Kept::S3 {
            return self.inner.write(place, guard, at, part, vault_key).await;
        }
        let key = object_key(at, part.offset)?;
        self.objects.put(&key, part.bytes).await?;
        let size = i64::try_from(part.bytes.len()).map_err(|_| unreadable("part size"))?;
        guard.extend(located(at));
        guard.extend([
            bind("offset", part.offset),
            bind("end", part.end),
            bind("key", key.as_str()),
            bind("size", size),
        ]);
        let record = match part.completes {
            Some(audit) => {
                guard.extend(audit);
                RECORD
            }
            None => "",
        };
        let script = format!(
            "BEGIN; {GUARD} {SHARES_UNCHANGED} {IN_ORDER} \
             CREATE file_objects = {{ company: $company, file: $file, offset: $offset, object_key: $key, size: $size }}; \
             UPDATE files:$file SET written = $end; {record} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", guard)
            .await
            .map_err(|error| thrown(error.into()))?;
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
        if kept != Kept::S3 {
            return self
                .inner
                .read(place, guard, at, kept, (offset, limit), vault_key)
                .await;
        }
        guard.extend(located(at));
        let recorded = format!("BEGIN; {GUARD} {SHARES_UNCHANGED} {RECORD} COMMIT;");
        let first = part_index(offset)?
            .checked_mul(super::tessaridb::part_size()?)
            .ok_or_else(|| unreadable("first part"))?;
        let last = part_index(offset.saturating_add(limit).saturating_sub(1))?
            .checked_mul(super::tessaridb::part_size()?)
            .ok_or_else(|| unreadable("last part"))?;
        guard.extend([bind("first", first), bind("last", last)]);
        let answers = self
            .database
            .transact_at(
                place,
                &recorded,
                "SELECT offset, object_key, size FROM file_objects WHERE company = $company AND file = $file AND offset >= $first AND offset <= $last ORDER BY offset;",
                guard,
            )
            .await
            .map_err(|error| thrown(error.into()))?;
        let selected = answers.into_iter().last().into_iter().collect();
        let objects = records::<FileObject>(selected, "file object")?;
        let span = last
            .checked_sub(first)
            .ok_or_else(|| unreadable("part span"))?;
        let expected = span
            .checked_div(super::tessaridb::part_size()?)
            .and_then(|parts| parts.checked_add(1))
            .ok_or_else(|| unreadable("part count"))?;
        if objects.len() != usize::try_from(expected).map_err(|_| unreadable("part count"))? {
            return Err(unreadable("published part count"));
        }
        let mut whole = Vec::new();
        for object in objects {
            let bytes = self.objects.get(&object.key).await?;
            if bytes.len() != usize::try_from(object.size).map_err(|_| unreadable("object size"))? {
                return Err(unreadable("object size"));
            }
            whole.extend_from_slice(&bytes);
        }
        let skipped = offset
            .checked_sub(first)
            .ok_or_else(|| unreadable("range start"))?;
        Ok(slice(&whole, skipped, limit))
    }

    async fn delete(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        file: &FileEntity,
        vault_key: VaultKey,
    ) -> Result<()> {
        if file.kept != Kept::S3.word() {
            return self.inner.delete(place, guard, at, file, vault_key).await;
        }
        guard.extend(located(at));
        let answers = self
            .database
            .read_at(
                place,
                "SELECT offset, object_key, size FROM file_objects WHERE company = $company AND file = $file;",
                guard.clone(),
            )
            .await?;
        let objects = records::<FileObject>(answers, "file object")?;
        let script = format!(
            "BEGIN; {GUARD} THROW 'not found' IF (GET files:$file).author != $author \
             OR (GET files:$file).record != $record; DELETE FROM file_objects WHERE company = $company AND file = $file; \
             DELETE files:$file; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", guard)
            .await
            .map_err(|error| thrown(error.into()))?;
        for object in objects {
            let _ = self.objects.delete(&object.key).await;
        }
        Ok(())
    }
}

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
