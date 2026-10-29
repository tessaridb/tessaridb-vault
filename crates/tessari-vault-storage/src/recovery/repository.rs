//! What recovery needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId};

use super::entity::RecoveryKeyEntity;
use crate::Result;
use crate::group_keys::NewGroupKey;
use crate::keys::KeyEntity;

#[async_trait]
pub(crate) trait RecoveryRepository: Send + Sync {
    /// The company's recovery key, if an owner has made one.
    async fn get(&self, place: &Place, company: &CompanyId) -> Result<Option<RecoveryKeyEntity>>;

    /// The key sets of the company's active owners.
    async fn owners(&self, place: &Place, company: &CompanyId) -> Result<Vec<KeyEntity>>;

    /// Keep the recovery key and its owners' wraps, once, under `guard`.
    async fn keep(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        key: &NewGroupKey,
        vault_key: VaultKey,
    ) -> Result<()>;

    /// The wraps of the recovery key by the owner key id they are wrapped to; the read is on the trail first.
    async fn wraps(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        vault_key: VaultKey,
    ) -> Result<Vec<(String, Vec<u8>)>>;

    /// The key id of `account`'s key set in the company, if they keep one.
    async fn own_key(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>>;
}
