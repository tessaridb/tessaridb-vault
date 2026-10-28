//! The `chains` and `statements` rows and what they read as.

use tessari_vault_constants::{
    CHAIN_MAX, MAX_STATEMENT_BYTES, PUBLIC_KEY_BYTES, STATEMENT_SIGNATURE_BYTES,
};
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::base64url;

use crate::{Error, Result};

/// One `chains` row: the owner key the genesis pinned and how many statements follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainEntity {
    pub(crate) owner_signing: String,
    pub(crate) length: i64,
}

impl FromRecord for ChainEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            owner_signing: row.take("owner_signing")?,
            length: row.take("length")?,
        })
    }
}

/// One `statements` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatementEntity {
    pub(crate) seq: i64,
    pub(crate) bytes: String,
    pub(crate) signature: String,
}

impl FromRecord for StatementEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            seq: row.take("seq")?,
            bytes: row.take("bytes")?,
            signature: row.take("signature")?,
        })
    }
}

/// A signed statement at its place in the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    seq: u64,
    bytes: Vec<u8>,
    signature: Vec<u8>,
}

impl Statement {
    /// A statement at `seq`, its parts checked against their ceilings.
    ///
    /// # Errors
    ///
    /// When `seq` is outside the chain's length, the bytes are empty or too long, or the signature is not 64 bytes.
    pub fn new(seq: u64, bytes: Vec<u8>, signature: Vec<u8>) -> Result<Self> {
        let shaped = (1..=CHAIN_MAX).contains(&seq)
            && !bytes.is_empty()
            && bytes.len() <= MAX_STATEMENT_BYTES
            && signature.len() == STATEMENT_SIGNATURE_BYTES;
        if shaped {
            Ok(Self {
                seq,
                bytes,
                signature,
            })
        } else {
            Err(Error::Unreadable {
                record: "statement",
                reason: "a part outside its shape or ceiling".to_owned(),
            })
        }
    }

    /// Its place in the chain, from 1.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The signed bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The signature over them.
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }
}

/// A company's whole chain, or none when no owner has begun one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chain {
    /// The owner's Ed25519 public key the genesis pinned.
    pub owner_signing: Option<Vec<u8>>,
    /// Every statement, in order.
    pub statements: Vec<Statement>,
}

fn fault(what: &str) -> Error {
    Error::Unreadable {
        record: "statement",
        reason: format!("{what} did not read back as written"),
    }
}

/// The pinned owner key, checked as a public key.
pub(crate) fn pin(owner_signing: Vec<u8>) -> Result<Vec<u8>> {
    if owner_signing.len() == PUBLIC_KEY_BYTES {
        Ok(owner_signing)
    } else {
        Err(Error::Unreadable {
            record: "statement",
            reason: "the owner key is not a public key".to_owned(),
        })
    }
}

impl StatementEntity {
    pub(crate) fn into_model(self) -> Result<Statement> {
        Statement::new(
            u64::try_from(self.seq).map_err(|_| fault("seq"))?,
            base64url::decode(&self.bytes).map_err(|_| fault("bytes"))?,
            base64url::decode(&self.signature).map_err(|_| fault("signature"))?,
        )
        .map_err(|_| fault("statement"))
    }
}

impl ChainEntity {
    pub(crate) fn owner_signing(&self) -> Result<Vec<u8>> {
        base64url::decode(&self.owner_signing).map_err(|_| fault("owner key"))
    }
}
