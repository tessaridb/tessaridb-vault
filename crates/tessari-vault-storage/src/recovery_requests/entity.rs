//! A `recovery_requests` row and what it reads as.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, KeyId, base64url};

use crate::{Error, Result};

/// One open `recovery_requests` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestEntity {
    pub(crate) id: String,
    pub(crate) account: String,
    pub(crate) handoff_public: String,
    pub(crate) created: i64,
}

impl FromRecord for RequestEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("request_id")?,
            account: row.take("account")?,
            handoff_public: row.take("handoff_public")?,
            created: row.take("created")?,
        })
    }
}

/// A request an owner may hand over: whose it is and the handoff key to wrap to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    /// The request, which is also the id the handoff wrap is bound to.
    pub id: KeyId,
    /// The member asking.
    pub account: AccountId,
    /// The member's one-time X25519 handoff key.
    pub handoff_public: Vec<u8>,
    /// When the member asked, in seconds since the epoch.
    pub created: i64,
}

impl RequestEntity {
    pub(crate) fn into_model(self) -> Result<OpenRequest> {
        let fault = |what: &str| Error::Unreadable {
            record: "recovery request",
            reason: format!("{what} did not read back as written"),
        };
        Ok(OpenRequest {
            id: KeyId::parse(&self.id).map_err(|_| fault("id"))?,
            account: AccountId::parse(&self.account).map_err(|_| fault("account"))?,
            handoff_public: base64url::decode(&self.handoff_public)
                .map_err(|_| fault("handoff key"))?,
            created: self.created,
        })
    }
}
