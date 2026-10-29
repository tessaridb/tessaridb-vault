//! Encrypted files on records (C5, LR-VA-004). Every call decides first, then reaches the file only through its
//! record: a member the record reaches reads its files, one who edits it adds and writes them, and anyone else's
//! file is answered as one that does not exist.

use std::sync::Arc;

use tessari_vault_constants::FILE_PART_MAX_BYTES;
use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place, RefusalClass};
use tessari_vault_types::{CompanyId, FileId, RecordId};

use super::entity::{FileEntity, FileMeta, Kept};
use super::repository::{FileRef, FileRepository, Part};
use crate::access::{self, Actor, guard_values, recorded};
use crate::audit::epoch_seconds;
use crate::records::{RecordService, RecordVaults};
use crate::{Conflict, Error, Result};

/// Files.
pub struct FileService {
    repository: Arc<dyn FileRepository>,
    records: Arc<RecordService>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
}

fn count(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| Error::Conflict(Conflict::OutOfOrder))
}

impl FileService {
    pub(crate) fn new(
        repository: Arc<dyn FileRepository>,
        records: Arc<RecordService>,
        vaults: Arc<RecordVaults>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            records,
            vaults,
            database,
            place,
        }
    }

    /// Start a file of `size` sealed bytes on a record the caller edits.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision and for a caller who only reads the record; [`Error::NotFound`] for a
    /// record the caller does not reach.
    pub async fn create(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        size: u64,
    ) -> Result<FileMeta> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReviseRecord,
        )
        .await?;
        let (entity, reached) = self.records.reached(actor, company, record).await?;
        reached.editing()?;
        let file = FileId::from_random(random::bytes()?);
        let kept = Kept::for_size(size);
        let at = FileRef {
            company,
            record,
            file: &file,
            author: actor.account(),
            share_version: entity.share_version,
        };
        self.repository
            .create(
                &self.place,
                recorded(actor, company, &standing, "file.create", file.as_str()),
                &at,
                count(size)?,
                kept,
                epoch_seconds(),
            )
            .await?;
        Ok(FileMeta {
            id: file,
            size,
            kept,
            written: 0,
        })
    }

    /// Write `bytes` at `offset`, which must be where the file's written bytes end; a file kept in the vault is
    /// written whole in one part, and a bucket file in parts of exactly [`FILE_PART_MAX_BYTES`] but the last.
    ///
    /// # Errors
    ///
    /// As [`FileService::create`], and [`Conflict::OutOfOrder`] for a part not at the end of what is written, past
    /// the file's size, of the wrong length, or racing another part.
    pub async fn write(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        file: &FileId,
        offset: u64,
        bytes: &[u8],
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReviseRecord,
        )
        .await?;
        let (entity, reached) = self.records.reached(actor, company, record).await?;
        reached.editing()?;
        let at = FileRef {
            company,
            record,
            file,
            author: actor.account(),
            share_version: entity.share_version,
        };
        let meta = self.found(&at).await?;
        let end = u64::try_from(bytes.len())
            .ok()
            .and_then(|length| offset.checked_add(length))
            .ok_or(Error::Conflict(Conflict::OutOfOrder))?;
        let whole = offset == 0 && end == meta.size;
        let in_order = offset == meta.written && end <= meta.size && !bytes.is_empty();
        // A bucket file is one object per part, so every part but the last is exactly a part long.
        let part_sized = end == meta.size || end.checked_sub(offset) == Some(FILE_PART_MAX_BYTES);
        let shaped = match meta.kept {
            Kept::Vault => whole,
            Kept::Bucket => part_sized,
        };
        if !in_order || !shaped {
            return Err(Error::Conflict(Conflict::OutOfOrder));
        }
        let completes = (end == meta.size)
            .then(|| recorded(actor, company, &standing, "file.complete", file.as_str()));
        let guard = match &completes {
            Some(_) => Vec::new(),
            None => guard_values(actor, company, &standing),
        };
        let written = self
            .repository
            .write(
                &self.place,
                guard,
                &at,
                Part {
                    kept: meta.kept,
                    offset: count(offset)?,
                    end: count(end)?,
                    bytes,
                    completes,
                },
                self.vaults.key(company, standing.key_epoch)?,
            )
            .await;
        match written {
            // Two parts at one offset: the store reports the loser as contention, which is out of order here.
            Err(Error::Store(tessari_vault_infrastructure::Error::Refused {
                class: Some(RefusalClass::Retry),
                ..
            })) => Err(Error::Conflict(Conflict::OutOfOrder)),
            other => other,
        }
    }

    /// At most `limit` bytes of a whole file from `offset`; the read is on the company's trail before the bytes are.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a file on a record the caller does not reach;
    /// [`Conflict::Incomplete`] for a file not yet written to its size.
    pub async fn read(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        file: &FileId,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<u8>> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadRecord,
        )
        .await?;
        let (entity, _) = self.records.reached(actor, company, record).await?;
        let at = FileRef {
            company,
            record,
            file,
            author: actor.account(),
            share_version: entity.share_version,
        };
        let meta = self.found(&at).await?;
        if meta.written != meta.size {
            return Err(Error::Conflict(Conflict::Incomplete));
        }
        self.repository
            .read(
                &self.place,
                recorded(actor, company, &standing, "file.read", file.as_str()),
                &at,
                meta.kept,
                (count(offset)?, count(limit)?),
                self.vaults.key(company, standing.key_epoch)?,
            )
            .await
    }

    async fn found(&self, at: &FileRef<'_>) -> Result<FileMeta> {
        self.repository
            .get(&self.place, at)
            .await?
            .ok_or(Error::NotFound)
            .and_then(FileEntity::into_model)
    }
}
