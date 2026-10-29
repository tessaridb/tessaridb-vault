//! What recovery needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId};

use super::entity::{EscrowEntity, NoticeEntity, RecoveryKeyEntity};
use crate::Result;
use crate::group_keys::{MemberWrap, NewGroupKey};
use crate::keys::KeyEntity;

/// A member's private keys wrapped by their browser to the recovery key `recovery_key_id`.
pub(crate) struct Escrowed<'a> {
    pub(crate) account: &'a AccountId,
    pub(crate) recovery_key_id: &'a str,
    pub(crate) wrap: &'a [u8],
}

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

    /// The wraps of the recovery key by the owner key id they are wrapped to; with `audit`, the read is on the trail
    /// first.
    async fn wraps(
        &self,
        place: &Place,
        audit: Option<Vec<(String, Value)>>,
        company: &CompanyId,
        vault_key: VaultKey,
    ) -> Result<Vec<(String, Vec<u8>)>>;

    /// Add `wraps` of the recovery key to owners' key sets, under `guard`.
    async fn add_wraps(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        wraps: &[MemberWrap],
        vault_key: VaultKey,
    ) -> Result<()>;

    /// The key id of `account`'s key set in the company, if they keep one.
    async fn own_key(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>>;

    /// Keep a member's escrow, replacing an earlier one.
    async fn keep_escrow(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        escrow: Escrowed<'_>,
        vault_key: VaultKey,
    ) -> Result<()>;

    /// `account`'s escrow row, if they keep one: the recovery key it is wrapped to, never the wrap.
    async fn escrow_row(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<EscrowEntity>>;

    /// `account`'s escrow, read by `by`: the read and a notice to `account` commit before it is opened.
    async fn read_escrow(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        vault_key: VaultKey,
    ) -> Result<(String, Vec<u8>)>;

    /// The notices about `account` in the company, oldest first.
    async fn notices(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Vec<NoticeEntity>>;
}
