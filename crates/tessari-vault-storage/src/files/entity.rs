//! A `files` row and what it reads as.

use tessari_vault_constants::MAX_VAULT_FILE_BYTES;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::FileId;

use crate::{Error, Result};

/// One `files` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileEntity {
    pub(crate) id: String,
    pub(crate) size: i64,
    pub(crate) kept: String,
    pub(crate) written: i64,
}

impl FromRecord for FileEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("file_id")?,
            size: row.take("size")?,
            kept: row.take("kept")?,
            written: row.take("written")?,
        })
    }
}

/// Where a file's sealed bytes are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// In the company's records vault, under its key: two layers.
    Vault,
    /// In the files bucket: the browser's layer only.
    Bucket,
}

impl Kept {
    /// Where a file of `size` bytes is kept.
    #[must_use]
    pub const fn for_size(size: u64) -> Self {
        if size <= MAX_VAULT_FILE_BYTES {
            Self::Vault
        } else {
            Self::Bucket
        }
    }

    /// The word it is stored and sent as.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Vault => "vault",
            Self::Bucket => "bucket",
        }
    }
}

/// A file as its record's author sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    /// Its id.
    pub id: FileId,
    /// Its sealed size in bytes.
    pub size: u64,
    /// Where it is kept.
    pub kept: Kept,
    /// How many of its bytes have been written, from the start.
    pub written: u64,
}

impl FileEntity {
    pub(crate) fn into_model(self) -> Result<FileMeta> {
        let fault = |what: &str| Error::Unreadable {
            record: "file",
            reason: format!("{what} did not read back as written"),
        };
        let kept = match self.kept.as_str() {
            "vault" => Kept::Vault,
            "bucket" => Kept::Bucket,
            _ => return Err(fault("kept")),
        };
        Ok(FileMeta {
            id: FileId::parse(&self.id).map_err(|_| fault("id"))?,
            size: u64::try_from(self.size).map_err(|_| fault("size"))?,
            kept,
            written: u64::try_from(self.written).map_err(|_| fault("written"))?,
        })
    }
}
