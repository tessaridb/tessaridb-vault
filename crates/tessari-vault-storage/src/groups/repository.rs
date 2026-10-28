//! What the groups service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value};
use tessari_vault_types::{AccountId, CompanyId, GroupId, GroupKind, Name};

use super::entity::{GroupEntity, MemberRow};
use crate::Result;

/// A department or team about to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewGroup {
    pub(crate) id: GroupId,
    pub(crate) kind: GroupKind,
    pub(crate) name: Name,
    pub(crate) parent: GroupId,
}

#[async_trait]
pub(crate) trait GroupRepository: Send + Sync {
    async fn list(&self, place: &Place, company: &CompanyId) -> Result<Vec<GroupEntity>>;
    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Option<GroupEntity>>;
    async fn members(&self, place: &Place, group: &GroupId) -> Result<Vec<MemberRow>>;
    async fn create(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &NewGroup,
    ) -> Result<()>;
    async fn rename(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        name: &Name,
    ) -> Result<()>;
    async fn delete(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupEntity,
        members: &[AccountId],
    ) -> Result<()>;
    async fn add_member(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        account: &AccountId,
    ) -> Result<()>;
    async fn remove_member(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        account: &AccountId,
    ) -> Result<()>;
}
