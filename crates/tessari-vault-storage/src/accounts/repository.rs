//! What the accounts service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::Place;
use tessari_vault_types::{AccountId, Email};

use super::AccountEntity;
use crate::Result;

#[async_trait]
pub(crate) trait AccountRepository: Send + Sync {
    /// The account with this id.
    async fn get(&self, place: &Place, id: &AccountId) -> Result<Option<AccountEntity>>;
    /// The account with this address.
    async fn find_by_email(&self, place: &Place, email: &Email) -> Result<Option<AccountEntity>>;
    /// The accounts with these ids, in no particular order.
    async fn get_many(&self, place: &Place, ids: &[AccountId]) -> Result<Vec<AccountEntity>>;
    /// Create a platform admin's account.
    async fn create_admin(
        &self,
        place: &Place,
        id: &AccountId,
        email: &Email,
        name: &str,
    ) -> Result<()>;
    /// Make an existing account a platform admin.
    async fn make_admin(&self, place: &Place, id: &AccountId) -> Result<()>;
}
