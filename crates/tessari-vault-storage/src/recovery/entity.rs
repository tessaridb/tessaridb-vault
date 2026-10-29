//! A `recovery_keys` row and what it reads as.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, KeyId, base64url};

use crate::{Error, Result};

/// The company's `recovery_keys` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryKeyEntity {
    pub(crate) key_id: String,
    pub(crate) public: String,
    /// Set when an owner stops being one: the key retires once an owner makes the next.
    pub(crate) rotate: bool,
}

impl FromRecord for RecoveryKeyEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            key_id: row.take("key_id")?,
            public: row.take("public")?,
            rotate: row.take("rotate")?,
        })
    }
}

/// A member's `escrows` row: which recovery key their escrow is wrapped to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EscrowEntity {
    pub(crate) recovery_key_id: String,
}

impl FromRecord for EscrowEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            recovery_key_id: row.take("recovery_key_id")?,
        })
    }
}

/// The company's recovery key as a member reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryKey {
    /// The id escrow wraps to it are named by.
    pub key_id: KeyId,
    /// X25519 public key.
    pub public: Vec<u8>,
    /// An owner stopped being one since it was made, so an owner makes the next.
    pub rotation_due: bool,
}

impl RecoveryKeyEntity {
    pub(crate) fn into_model(self) -> Result<RecoveryKey> {
        let fault = |what: &str| Error::Unreadable {
            record: "recovery key",
            reason: format!("{what} did not read back as written"),
        };
        Ok(RecoveryKey {
            key_id: KeyId::parse(&self.key_id).map_err(|_| fault("key id"))?,
            public: base64url::decode(&self.public).map_err(|_| fault("public key"))?,
            rotation_due: self.rotate,
        })
    }
}

/// One `recovery_notices` row: an act on a member's escrow, kept for that member to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoticeEntity {
    pub(crate) by_account: String,
    pub(crate) kind: String,
    pub(crate) created: i64,
}

impl FromRecord for NoticeEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            by_account: row.take("by_account")?,
            kind: row.take("kind")?,
            created: row.take("created")?,
        })
    }
}

/// An act on the caller's escrow: who took it, what it was, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The owner who took it.
    pub by: AccountId,
    /// What was done: `escrow.read`.
    pub kind: String,
    /// When, in seconds since the epoch.
    pub at: i64,
}

impl NoticeEntity {
    pub(crate) fn into_model(self) -> Result<Notice> {
        Ok(Notice {
            by: AccountId::parse(&self.by_account).map_err(|_| Error::Unreadable {
                record: "recovery notice",
                reason: "account did not read back as written".to_owned(),
            })?,
            kind: self.kind,
            at: self.created,
        })
    }
}
