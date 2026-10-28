//! What sharing and reaching need from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, RecordId};

use super::entity::{DirectGroup, NewShare, ShareEntity};
use crate::Result;

/// One share about to be written onto `revision` of `record`, at the record's share version `share_version`.
pub(crate) struct ShareWrite<'a> {
    pub(crate) company: &'a CompanyId,
    pub(crate) record: &'a RecordId,
    pub(crate) revision: i64,
    pub(crate) share_version: i64,
    pub(crate) share: &'a NewShare,
    pub(crate) by: &'a AccountId,
    pub(crate) now: i64,
    pub(crate) vault_key: VaultKey,
}

/// One share about to be removed from `record`, read at `revision` and the share version `share_version`.
pub(crate) struct ShareRemoval<'a> {
    pub(crate) company: &'a CompanyId,
    pub(crate) record: &'a RecordId,
    pub(crate) revision: i64,
    pub(crate) share_version: i64,
    pub(crate) target: &'a str,
    pub(crate) vault_key: VaultKey,
}

#[async_trait]
pub(crate) trait ShareRepository: Send + Sync {
    /// The groups `account` is directly in, wherever they are.
    async fn direct_groups(&self, place: &Place, account: &AccountId) -> Result<Vec<DirectGroup>>;
    /// Every share of `record`.
    async fn of_record(&self, place: &Place, record: &str) -> Result<Vec<ShareEntity>>;
    /// The ids of `company`'s records shared with `target`: the predicate is in the query.
    async fn shared_with(
        &self,
        place: &Place,
        company: &CompanyId,
        target: &str,
    ) -> Result<Vec<String>>;
    /// The id of the key set `account` keeps in `company`, if any.
    async fn member_key(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<String>>;
    /// Write the share row and its wrap as a recipient of the record's revision, in one transaction that refuses a
    /// target outside the company, a key that is not the target's, a second share to one target, and a record
    /// that moved since it was read.
    async fn create(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        write: ShareWrite<'_>,
    ) -> Result<()>;
    /// Remove the share row and its wrap from the record's latest revision, and mark the record for rotation, in one
    /// transaction that refuses a share that does not exist and a record that moved since it was read.
    async fn remove(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        removal: ShareRemoval<'_>,
    ) -> Result<()>;
}
