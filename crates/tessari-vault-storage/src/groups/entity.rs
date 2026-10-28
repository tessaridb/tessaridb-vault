//! A department or team row and what it reads as.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, GroupId, GroupKind, Name};

use crate::{Error, Result};

/// One `groups` row of kind `department` or `team`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GroupEntity {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) parent: String,
}

impl FromRecord for GroupEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("group_id")?,
            kind: row.take("kind")?,
            name: row.take("name")?,
            parent: row.take("parent")?,
        })
    }
}

/// A department or a team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Its id.
    pub id: GroupId,
    /// Department or team.
    pub kind: GroupKind,
    /// Its name.
    pub name: Name,
    /// The group it sits in: the company's own group, or a department.
    pub parent: GroupId,
}

impl GroupEntity {
    pub(crate) fn into_model(self) -> Result<Group> {
        let fault = |what: &str| Error::Unreadable {
            record: "group",
            reason: format!("{what} did not read back as written"),
        };
        Ok(Group {
            id: GroupId::parse(&self.id).map_err(|_| fault("id"))?,
            kind: self.kind.parse().map_err(|_| fault("kind"))?,
            name: Name::parse(&self.name).map_err(|_| fault("name"))?,
            parent: GroupId::parse(&self.parent).map_err(|_| fault("parent"))?,
        })
    }
}

/// One account id read off a group's member edges.
pub(crate) struct MemberRow {
    pub(crate) account: String,
}

impl FromRecord for MemberRow {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            account: row.take("account_id")?,
        })
    }
}

impl MemberRow {
    pub(crate) fn into_id(self) -> Result<AccountId> {
        AccountId::parse(&self.account).map_err(|_| Error::Unreadable {
            record: "group member",
            reason: "an account id did not read back as written".to_owned(),
        })
    }
}
