//! What recovery requests need from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, KeyId};

use super::entity::RequestEntity;
use crate::Result;
use crate::keys::Kdf;

/// A member asking to be recovered: who, the one-time handoff key, and when the request closes.
pub(crate) struct Asking<'a> {
    pub(crate) account: &'a AccountId,
    pub(crate) handoff_public: &'a [u8],
    pub(crate) expires: i64,
}

/// The member's keys wrapped again by their browser under a new unlock key, using up the redeemed `request`.
pub(crate) struct Rewrapped<'a> {
    pub(crate) request: &'a KeyId,
    pub(crate) kdf: &'a Kdf,
    pub(crate) wrapped: &'a [u8],
}

#[async_trait]
pub(crate) trait RecoveryRequestRepository: Send + Sync {
    /// Open request `id` for the asking member; refused when they keep no escrow.
    async fn open(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        id: &KeyId,
        asking: Asking<'_>,
    ) -> Result<()>;

    /// The requests in the company no owner has handed over and that have not expired at `now`.
    async fn open_requests(
        &self,
        place: &Place,
        company: &CompanyId,
        now: i64,
    ) -> Result<Vec<RequestEntity>>;

    /// Keep the owner's handoff `wrap` for request `id`, with a notice to the member, once.
    async fn hand_over(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        id: &KeyId,
        wrap: &[u8],
        vault_key: VaultKey,
    ) -> Result<()>;

    /// Mark request `id` of `account` redeemed and answer its handoff wrap, once.
    async fn redeem(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        id: &KeyId,
        vault_key: VaultKey,
    ) -> Result<Vec<u8>>;

    /// Write the audit row `guard` carries and nothing else.
    async fn record(&self, place: &Place, guard: Vec<(String, Value)>) -> Result<()>;

    /// Replace `account`'s wrapped keys and unlock parameters.
    async fn rewrap(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        account: &AccountId,
        keys: Rewrapped<'_>,
        vault_key: VaultKey,
    ) -> Result<()>;
}
