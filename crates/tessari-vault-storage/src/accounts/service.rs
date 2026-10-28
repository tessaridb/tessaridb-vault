//! Reading accounts, and the one start-up act that names a platform admin.

use std::sync::Arc;

use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::Place;
use tessari_vault_types::{AccountId, Email, Name};

use super::Account;
use super::repository::AccountRepository;
use crate::access::Actor;
use crate::{Error, Result};

/// Accounts.
pub struct AccountService {
    repository: Arc<dyn AccountRepository>,
    place: Place,
}

impl AccountService {
    pub(crate) fn new(repository: Arc<dyn AccountRepository>, place: Place) -> Self {
        Self { repository, place }
    }

    /// The caller's own account.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] when the account behind a live session is gone; [`Error::Store`] when the store fails.
    pub async fn me(&self, actor: &Actor) -> Result<Account> {
        self.repository
            .get(&self.place, actor.account())
            .await?
            .ok_or(Error::NotFound)?
            .into_model()
    }

    /// Make the account with `email` a platform admin, creating it when the address is new.
    ///
    /// A bypass by design: it is run by the node at start-up from its own configuration, never from a request, and
    /// is the only way a platform admin comes to exist.
    ///
    /// # Errors
    ///
    /// When the store fails or randomness is unavailable.
    pub async fn system_ensure_platform_admin(
        &self,
        email: &Email,
        name: &Name,
    ) -> Result<AccountId> {
        if let Some(found) = self.repository.find_by_email(&self.place, email).await? {
            let account = found.into_model()?;
            if !account.platform_admin {
                self.repository.make_admin(&self.place, &account.id).await?;
            }
            return Ok(account.id);
        }
        let id = AccountId::from_random(random::bytes()?);
        self.repository
            .create_admin(&self.place, &id, email, name.as_str())
            .await?;
        Ok(id)
    }

    /// The account with `email`, if one exists.
    pub(crate) async fn find_by_email(&self, email: &Email) -> Result<Option<Account>> {
        self.repository
            .find_by_email(&self.place, email)
            .await?
            .map(super::AccountEntity::into_model)
            .transpose()
    }

    /// The accounts with these ids.
    pub(crate) async fn many(&self, ids: &[AccountId]) -> Result<Vec<Account>> {
        self.repository
            .get_many(&self.place, ids)
            .await?
            .into_iter()
            .map(super::AccountEntity::into_model)
            .collect()
    }

    /// The account with this id, if one exists.
    pub(crate) async fn get(&self, id: &AccountId) -> Result<Option<Account>> {
        self.repository
            .get(&self.place, id)
            .await?
            .map(super::AccountEntity::into_model)
            .transpose()
    }
}
