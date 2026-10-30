//! A `files` row and what it reads as.

use tessari_vault_constants::MAX_VAULT_FILE_BYTES;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::FileId;

use crate::{Error, Result};

/// One S3 object key published as a file part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileObject {
    pub(crate) offset: i64,
    pub(crate) key: String,
    pub(crate) size: i64,
}

impl FromRecord for FileObject {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            offset: row.take("offset")?,
            key: row.take("object_key")?,
            size: row.take("size")?,
        })
    }
}

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
    /// In TessariDB S3: the browser's layer only.
    S3,
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

    /// Where a file of `size` bytes is kept when S3 is optionally enabled.
    #[must_use]
    pub const fn for_size_with_s3(size: u64, s3: bool) -> Self {
        match (size <= MAX_VAULT_FILE_BYTES, s3) {
            (true, _) => Self::Vault,
            (false, false) => Self::Bucket,
            (false, true) => Self::S3,
        }
    }

    /// The word it is stored and sent as.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Vault => "vault",
            Self::Bucket => "bucket",
            Self::S3 => "s3",
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
            "s3" => Kept::S3,
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

#[cfg(test)]
mod tests {
    use super::{FileEntity, Kept};

    #[test]
    fn every_backend_word_reads_back_as_written() {
        for (word, expected) in [
            ("vault", Kept::Vault),
            ("bucket", Kept::Bucket),
            ("s3", Kept::S3),
        ] {
            let meta = FileEntity {
                id: "ab".repeat(16),
                size: 10,
                kept: word.to_owned(),
                written: 0,
            }
            .into_model()
            .expect("a known backend");
            assert_eq!(meta.kept, expected);
            assert_eq!(meta.kept.word(), word);
        }
    }
}
