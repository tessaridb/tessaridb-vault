//! What the files service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, FileId, RecordId};

use super::entity::{FileEntity, Kept};
use crate::Result;

/// Which file, on which record, for whom: every statement checks all of it.
pub(crate) struct FileRef<'a> {
    pub(crate) company: &'a CompanyId,
    pub(crate) record: &'a RecordId,
    pub(crate) file: &'a FileId,
    pub(crate) author: &'a AccountId,
}

/// One part of a file about to be written at `offset`, ending at `end`.
pub(crate) struct Part<'a> {
    pub(crate) kept: Kept,
    pub(crate) offset: i64,
    pub(crate) end: i64,
    pub(crate) bytes: &'a [u8],
    /// The trail entry's values when this part completes the file; `None` writes no entry.
    pub(crate) completes: Option<Vec<(String, Value)>>,
}

#[async_trait]
pub(crate) trait FileRepository: Send + Sync {
    /// The file, when it is on `record`, the record is the company's, and `author` wrote it.
    async fn get(&self, place: &Place, at: &FileRef<'_>) -> Result<Option<FileEntity>>;
    /// Start a file of `size` sealed bytes on a record `author` wrote, refused as not found otherwise.
    async fn create(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        size: i64,
        kept: Kept,
        now: i64,
    ) -> Result<()>;
    /// Write `part`, refused as out of order unless the file holds exactly `part.offset` bytes.
    async fn write(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        part: Part<'_>,
        vault_key: VaultKey,
    ) -> Result<()>;
    /// Record the read on the trail, then read `limit` bytes from `offset`.
    async fn read(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        at: &FileRef<'_>,
        kept: Kept,
        range: (i64, i64),
        vault_key: VaultKey,
    ) -> Result<Vec<u8>>;
}
