//! What the records service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value, VaultKey};
use tessari_vault_types::{AccountId, CompanyId, KeyId, RecordId, RecordTypeId, Roles};

use super::entity::{RecordEntity, Sealed};
use crate::Result;

/// One revision about to be written, with the key that opens the company's vault for that one request.
pub(crate) struct Write<'a> {
    pub(crate) company: &'a CompanyId,
    pub(crate) record: &'a RecordId,
    pub(crate) revision: i64,
    pub(crate) record_type: &'a str,
    pub(crate) key_id: &'a KeyId,
    pub(crate) author: &'a AccountId,
    pub(crate) sealed: &'a Sealed,
    pub(crate) now: i64,
    pub(crate) vault_key: VaultKey,
    /// For a record of a company type: the type and the roles the decision admitted, which the write checks again.
    pub(crate) company_type: Option<(&'a RecordTypeId, Roles)>,
}

#[async_trait]
pub(crate) trait RecordRepository: Send + Sync {
    /// Every record of `company` that `author` wrote: the predicate is in the query, so nobody else's are read.
    async fn list(
        &self,
        place: &Place,
        company: &CompanyId,
        author: &AccountId,
    ) -> Result<Vec<RecordEntity>>;
    /// The record, when it is `company`'s and `author` wrote it.
    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        author: &AccountId,
        record: &RecordId,
    ) -> Result<Option<RecordEntity>>;
    /// Write a new record and its first revision.
    async fn create(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        write: Write<'_>,
    ) -> Result<()>;
    /// Write a revision over `expected`, refused when the record has moved past it.
    async fn revise(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        write: Write<'_>,
        expected: i64,
    ) -> Result<()>;
    /// Record the read on the trail, then open `entity`'s latest revision.
    async fn open(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        entity: &RecordEntity,
        vault_key: VaultKey,
    ) -> Result<Sealed>;
}
