//! A record row, a revision's sealed parts, and what they read as.

use std::collections::HashMap;

use tessari_vault_constants::{
    MAX_RECORD_BODY_BYTES, MAX_RECORD_WRAP_BYTES, RECORD_SIGNATURE_BYTES,
};
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, KeyId, RecordId, RecordType};

use crate::{Error, Result};

/// One `records` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordEntity {
    pub(crate) id: String,
    pub(crate) record_type: String,
    pub(crate) revision: i64,
    pub(crate) key_id: String,
    pub(crate) author: String,
    pub(crate) share_version: i64,
    pub(crate) rotate: bool,
    pub(crate) created: i64,
    pub(crate) updated: i64,
}

impl FromRecord for RecordEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("record_id")?,
            record_type: row.take("record_type")?,
            revision: row.take("revision")?,
            key_id: row.take("key_id")?,
            author: row.take("author")?,
            share_version: row.take("share_version")?,
            rotate: row.take("rotate")?,
            created: row.take("created")?,
            updated: row.take("updated")?,
        })
    }
}

/// A record as its author sees it: everything but its sealed parts, which the server cannot read either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Its id.
    pub id: RecordId,
    /// Its type.
    pub record_type: RecordType,
    /// Its latest revision, from 1.
    pub revision: i64,
    /// The id of the record key the latest revision is sealed under.
    pub key_id: KeyId,
    /// Who wrote it.
    pub author: AccountId,
    /// When it was created, in seconds since 1970.
    pub created: i64,
    /// When its latest revision was written, in seconds since 1970.
    pub updated: i64,
    /// Whether a share was removed since its latest revision, so the next one must be sealed under a new record key.
    pub rotation_due: bool,
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "record",
        reason: format!("{what} did not read back as written"),
    }
}

impl RecordEntity {
    /// Refuses a revision under the record key `key_id` while a removed share leaves the current key to retire.
    pub(crate) fn rotated_by(&self, key_id: &KeyId) -> Result<()> {
        if self.rotate && key_id.as_str() == self.key_id {
            Err(Error::Conflict(crate::Conflict::RotationDue))
        } else {
            Ok(())
        }
    }

    pub(crate) fn into_model(self) -> Result<Record> {
        Ok(Record {
            id: RecordId::parse(&self.id).map_err(|_| unreadable("id"))?,
            record_type: RecordType::parse(&self.record_type).map_err(|_| unreadable("type"))?,
            revision: self.revision,
            key_id: KeyId::parse(&self.key_id).map_err(|_| unreadable("key id"))?,
            author: AccountId::parse(&self.author).map_err(|_| unreadable("author"))?,
            created: self.created,
            updated: self.updated,
            rotation_due: self.rotate,
        })
    }
}

/// Why sealed parts are refused before anything is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a sealed body, a 66-byte signature and a wrapped record key within their ceilings")]
pub struct SealedError;

/// One revision's sealed parts, as the browser made them: the body sealed under the record key, the author's
/// signature over it at a position of the company's key chain, and the record key wrapped to the author's agreement
/// key. None of it is readable here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub(crate) body: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    pub(crate) author_key: KeyId,
    pub(crate) chain_seq: i64,
    pub(crate) wrap: Vec<u8>,
}

impl Sealed {
    /// The parts, each within its ceiling.
    ///
    /// # Errors
    ///
    /// [`SealedError`] for an empty or oversized body, a signature that is not 66 bytes, a negative chain position, or
    /// an empty or oversized wrap.
    pub fn new(
        body: Vec<u8>,
        signature: Vec<u8>,
        author_key: KeyId,
        chain_seq: i64,
        wrap: Vec<u8>,
    ) -> std::result::Result<Self, SealedError> {
        let fits = !body.is_empty()
            && chain_seq >= 0
            && body.len() <= MAX_RECORD_BODY_BYTES
            && signature.len() == RECORD_SIGNATURE_BYTES
            && !wrap.is_empty()
            && wrap.len() <= MAX_RECORD_WRAP_BYTES;
        if fits {
            Ok(Self {
                body,
                signature,
                author_key,
                chain_seq,
                wrap,
            })
        } else {
            Err(SealedError)
        }
    }

    /// The sealed body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// The author's signature over the revision.
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// The id of the agreement key the record key is wrapped to.
    #[must_use]
    pub const fn author_key(&self) -> &KeyId {
        &self.author_key
    }

    /// The position of the company's key chain the author signed at: the key vouched there is the one that signed.
    #[must_use]
    pub const fn chain_seq(&self) -> i64 {
        self.chain_seq
    }

    /// The record key, wrapped to the author's agreement key.
    #[must_use]
    pub fn wrap(&self) -> &[u8] {
        &self.wrap
    }
}

/// A record and its latest revision's sealed parts, as a member it reaches opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedRecord {
    /// The record.
    pub record: Record,
    /// Its latest revision's sealed parts, with the record key wrapped to a key the caller holds.
    pub sealed: Sealed,
    /// The recipient that wrap is kept under: `member:<key id>` or `group:<key id>`.
    pub recipient: String,
    /// The ids of the caller's own groups the record is shared with.
    pub through: Vec<String>,
    /// The account that wrote the latest revision, whose key signed it.
    pub revised_by: String,
}

/// A revision as the store opened it: the sealed body, the signature, the reviser's key, and every recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenedRevision {
    pub(crate) body: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    pub(crate) author_key: KeyId,
    pub(crate) chain_seq: i64,
    /// The account that wrote the revision, whose key signed it.
    pub(crate) author: String,
    pub(crate) recipients: HashMap<String, Vec<u8>>,
}

/// A revision as the caller wrote it: over `expected`, sealed under the record key `key_id`, and wrapping that key
/// to every recipient the record keeps but the caller.
pub struct Revision<'a> {
    /// The revision the caller read.
    pub expected: i64,
    /// The id of the record key the revision is sealed under.
    pub key_id: &'a KeyId,
    /// The sealed parts, with the caller's own wrap.
    pub sealed: &'a Sealed,
    /// The record key wrapped to each other recipient, by recipient name.
    pub wraps: &'a [(String, Vec<u8>)],
}
