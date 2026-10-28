//! The `accounts` row and the account it reads as.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, AccountState, Email, Name};

use crate::{Error, Result};

/// One `accounts` row as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountEntity {
    pub(crate) id: String,
    pub(crate) email: String,
    pub(crate) name: String,
    pub(crate) state: String,
    pub(crate) platform_admin: bool,
}

impl FromRecord for AccountEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("account_id")?,
            email: row.take("email")?,
            name: row.take("name")?,
            state: row.take("state")?,
            platform_admin: row.take("platform_admin")?,
        })
    }
}

/// An account: one person who signs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Its id.
    pub id: AccountId,
    /// The address that identifies the person.
    pub email: Email,
    /// The name shown for them.
    pub name: Name,
    /// Whether it may sign in.
    pub state: AccountState,
    /// Whether it manages companies on this server.
    pub platform_admin: bool,
}

impl AccountEntity {
    /// The account this row holds.
    pub(crate) fn into_model(self) -> Result<Account> {
        let fault = |what: &str| Error::Unreadable {
            record: "account",
            reason: format!("{what} did not read back as written"),
        };
        Ok(Account {
            id: AccountId::parse(&self.id).map_err(|_| fault("id"))?,
            email: Email::parse(&self.email).map_err(|_| fault("email"))?,
            name: Name::parse(&self.name).map_err(|_| fault("name"))?,
            state: self.state.parse().map_err(|_| fault("state"))?,
            platform_admin: self.platform_admin,
        })
    }
}
