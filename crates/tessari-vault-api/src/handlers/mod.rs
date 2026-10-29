//! The handlers: each parses what it was sent into typed values and calls one storage service, which decides.

pub(crate) mod audit;
pub(crate) mod companies;
pub(crate) mod enrolment;
pub(crate) mod files;
pub(crate) mod group_keys;
pub(crate) mod groups;
pub(crate) mod keys;
pub(crate) mod members;
pub(crate) mod oidc;
pub(crate) mod passkeys;
pub(crate) mod record_types;
pub(crate) mod records;
pub(crate) mod recovery;
pub(crate) mod shares;
pub(crate) mod sign_in;

use serde::Serialize;
use tessari_vault_storage::accounts::Account;
use tessari_vault_types::{AccountId, CompanyId, GroupId};

use crate::ApiError;

/// An account as another member of the company sees it.
#[derive(Serialize)]
pub(crate) struct AccountView {
    id: String,
    email: String,
    name: String,
}

impl From<&Account> for AccountView {
    fn from(account: &Account) -> Self {
        Self {
            id: account.id.to_string(),
            email: account.email.to_string(),
            name: account.name.to_string(),
        }
    }
}

// An id in a path that does not parse names nothing, so it is answered as a missing thing is.

pub(crate) fn company_id(text: &str) -> Result<CompanyId, ApiError> {
    CompanyId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}

pub(crate) fn group_id(text: &str) -> Result<GroupId, ApiError> {
    GroupId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}

pub(crate) fn account_id(text: &str) -> Result<AccountId, ApiError> {
    AccountId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}
