//! What the record types service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value};
use tessari_vault_types::{AccountId, CompanyId, RecordId, RecordTypeId, Roles};

use super::entity::RecordTypeEntity;
use crate::Result;

/// A type about to be defined.
pub(crate) struct NewType<'a> {
    pub(crate) company: &'a CompanyId,
    pub(crate) id: &'a RecordTypeId,
    pub(crate) allowed: Roles,
    pub(crate) definition: &'a RecordId,
    pub(crate) author: &'a AccountId,
    pub(crate) now: i64,
}

#[async_trait]
pub(crate) trait RecordTypeRepository: Send + Sync {
    async fn list(&self, place: &Place, company: &CompanyId) -> Result<Vec<RecordTypeEntity>>;
    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        id: &RecordTypeId,
    ) -> Result<Option<RecordTypeEntity>>;
    /// Define the type, refused as not found unless the definition is the author's own type-definition record in
    /// the company.
    async fn define(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        new: NewType<'_>,
    ) -> Result<()>;
}
