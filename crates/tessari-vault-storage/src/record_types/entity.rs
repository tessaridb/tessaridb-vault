//! A `record_types` row and what it reads as.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{RecordId, RecordTypeId, Roles};

use crate::{Error, Result};

/// One `record_types` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordTypeEntity {
    pub(crate) id: String,
    pub(crate) allowed: Vec<String>,
    pub(crate) definition: String,
    pub(crate) created: i64,
}

impl FromRecord for RecordTypeEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("type_id")?,
            allowed: row.take("allowed_roles")?,
            definition: row.take("definition")?,
            created: row.take("created")?,
        })
    }
}

/// A record type the company defined. Its name and fields are sealed in the definition record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTypeDefinition {
    /// Its id.
    pub id: RecordTypeId,
    /// Who may create records of it.
    pub allowed: Roles,
    /// The record holding its sealed definition.
    pub definition: RecordId,
    /// When it was defined, in seconds since 1970.
    pub created: i64,
}

impl RecordTypeEntity {
    pub(crate) fn into_model(self) -> Result<RecordTypeDefinition> {
        let fault = |what: &str| Error::Unreadable {
            record: "record type",
            reason: format!("{what} did not read back as written"),
        };
        Ok(RecordTypeDefinition {
            id: RecordTypeId::parse(&self.id).map_err(|_| fault("id"))?,
            allowed: Roles::parse(self.allowed.iter().map(String::as_str))
                .map_err(|_| fault("allowed roles"))?,
            definition: RecordId::parse(&self.definition).map_err(|_| fault("definition"))?,
            created: self.created,
        })
    }
}
