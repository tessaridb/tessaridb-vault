//! The rows the audit trail reads back.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};

/// One event as the trail shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Its place in the company's chain, from 1.
    pub seq: i64,
    /// When, in seconds since 1970.
    pub at: i64,
    /// Who: the account that acted.
    pub actor: String,
    /// What, as a stable dotted name.
    pub action: String,
    /// To whom or what.
    pub target: String,
    /// Why, as the actor said; empty when they gave no reason.
    pub reason: String,
}

impl FromRecord for AuditEntry {
    fn from_row(mut row: Row) -> Result<Self, MappingFault> {
        Ok(Self {
            seq: row.take("seq")?,
            at: row.take("at")?,
            actor: row.take("actor")?,
            action: row.take("action")?,
            target: row.take("target")?,
            reason: row.take("reason")?,
        })
    }
}

/// What the chain check reads of an event: its columns, and the line and digests stored beside them.
pub(super) struct Chained {
    pub(super) shown: AuditEntry,
    pub(super) company: String,
    pub(super) line: String,
    pub(super) prev: String,
    pub(super) digest: String,
}

impl FromRecord for Chained {
    fn from_row(mut row: Row) -> Result<Self, MappingFault> {
        let company = row.take("company")?;
        let line = row.take("line")?;
        let prev = row.take("prev")?;
        let digest = row.take("digest")?;
        Ok(Self {
            shown: AuditEntry::from_row(row)?,
            company,
            line,
            prev,
            digest,
        })
    }
}
