//! The rows passkeys read back.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};

pub(crate) use crate::enrolments::record::unhex;

/// The `credential` column of a `passkeys` row.
pub(super) struct StoredId {
    pub(super) credential: String,
}

impl FromRecord for StoredId {
    fn from_row(mut row: Row) -> Result<Self, MappingFault> {
        Ok(Self {
            credential: row.take("credential")?,
        })
    }
}

/// What a sign-in reads of a passkey.
pub(super) struct Stored {
    pub(super) account: String,
    pub(super) public_key: String,
    pub(super) sign_count: i64,
}

impl FromRecord for Stored {
    fn from_row(mut row: Row) -> Result<Self, MappingFault> {
        Ok(Self {
            account: row.take("account")?,
            public_key: row.take("public_key")?,
            sign_count: row.take("sign_count")?,
        })
    }
}
