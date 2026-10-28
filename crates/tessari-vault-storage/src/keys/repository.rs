//! What the keys service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId};

use super::entity::{KeyEntity, KeySet};
use crate::Result;

#[async_trait]
pub(crate) trait KeyRepository: Send + Sync {
    /// The member's key row in the company, if they registered one.
    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<KeyEntity>>;
    /// Every member's key row in the company.
    async fn list(&self, place: &Place, company: &CompanyId) -> Result<Vec<KeyEntity>>;
    /// Keep `set` as the member's, refused when they have one or its key id is taken in the company.
    async fn keep(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        set: &KeySet,
        now: i64,
        vault_key: VaultKey,
    ) -> Result<()>;
    /// Record the read on the trail, then open the member's wrapped private half.
    async fn open(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        vault_key: VaultKey,
    ) -> Result<Vec<u8>>;
}
