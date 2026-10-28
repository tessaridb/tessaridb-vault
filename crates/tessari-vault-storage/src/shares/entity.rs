//! A share row, the groups an account is in, and a share as a caller asks for one.

use tessari_vault_constants::MAX_RECORD_WRAP_BYTES;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, GroupId, KeyId};

/// What a share lets its target do with the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    /// Open the record and its files.
    Read,
    /// Also revise it, add files to it and share it on.
    Edit,
}

impl Access {
    /// The stored word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
        }
    }

    /// The access a stored or requested word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "read" => Some(Self::Read),
            "edit" => Some(Self::Edit),
            _ => None,
        }
    }
}

/// Whom a record is shared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareTarget {
    /// The company group, a department or a team, through the group's key.
    Group(GroupId),
    /// One member, through their own key set.
    Member(AccountId),
}

impl ShareTarget {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Group(_) => "group",
            Self::Member(_) => "member",
        }
    }

    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Group(group) => group.as_str(),
            Self::Member(account) => account.as_str(),
        }
    }
}

/// Why a share is refused before anything is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a share names its target's key and carries a wrapped record key within its ceiling")]
pub struct ShareError;

/// A share as the browser made it: the target, the access, the id of the key the record key is wrapped to, and the
/// wrap, which the server cannot open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewShare {
    pub(crate) target: ShareTarget,
    pub(crate) access: Access,
    pub(crate) key_id: KeyId,
    pub(crate) wrap: Vec<u8>,
}

impl NewShare {
    /// The share, its wrap within its ceiling.
    ///
    /// # Errors
    ///
    /// [`ShareError`] for an empty or oversized wrap.
    pub fn new(
        target: ShareTarget,
        access: Access,
        key_id: KeyId,
        wrap: Vec<u8>,
    ) -> Result<Self, ShareError> {
        if wrap.is_empty() || wrap.len() > MAX_RECORD_WRAP_BYTES {
            return Err(ShareError);
        }
        Ok(Self {
            target,
            access,
            key_id,
            wrap,
        })
    }

    /// The recipient name the wrap is kept under on the record's revision.
    pub(crate) fn recipient(&self) -> String {
        format!("{}:{}", self.target.kind(), self.key_id.as_str())
    }
}

/// One `shares` row, as far as reaching and revising need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShareEntity {
    pub(crate) target: String,
    pub(crate) access: String,
    pub(crate) recipient: String,
}

impl FromRecord for ShareEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            target: row.take("target")?,
            access: row.take("access")?,
            recipient: row.take("recipient")?,
        })
    }
}

/// A group an account is directly in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectGroup {
    pub(crate) group: String,
    pub(crate) company: String,
    pub(crate) parent: String,
}

impl FromRecord for DirectGroup {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            group: row.take("group_id")?,
            company: row.take("company")?,
            parent: row.take("parent")?,
        })
    }
}

/// One projected `name`.
pub(crate) struct Named {
    pub(crate) name: String,
}

impl FromRecord for Named {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            name: row.take("name")?,
        })
    }
}
