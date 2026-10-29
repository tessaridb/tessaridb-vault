//! What the group keys service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, GroupId};

use super::entity::{GroupKeyEntity, MemberWrap, NewGroupKey};
use crate::Result;
use crate::keys::KeyEntity;

#[async_trait]
pub(crate) trait GroupKeyRepository: Send + Sync {
    /// The key sets of every active member inside the group's reach, or `None` when the group is not the company's.
    async fn reach(
        &self,
        place: &Place,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Option<Vec<KeyEntity>>>;
    /// The group's key row, if it has one.
    async fn get(&self, place: &Place, group: &GroupId) -> Result<Option<GroupKeyEntity>>;
    /// Every wrap of the group key, by the member key id it is wrapped to; with `audit`, the read is on the trail
    /// first.
    async fn wraps(
        &self,
        place: &Place,
        audit: Option<Vec<(String, Value)>>,
        company: &CompanyId,
        group: &GroupId,
        vault_key: VaultKey,
    ) -> Result<Vec<(String, Vec<u8>)>>;
    /// Keep the group's key with its first wraps, refused when the group has one.
    async fn keep(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        key: &NewGroupKey,
        vault_key: VaultKey,
    ) -> Result<()>;
    /// Replace a group key due for rotation with `key` and its wraps: every old wrap goes, and the group's shares
    /// name the new key; refused unless the key is due.
    async fn rekey(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        key: &NewGroupKey,
        vault_key: VaultKey,
    ) -> Result<()>;
    /// Add wraps of the group's key.
    async fn add_wraps(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        wraps: &[MemberWrap],
        vault_key: VaultKey,
    ) -> Result<()>;
    /// The key id of the key set `account` keeps in the company, if any.
    async fn own_key(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>>;
}
