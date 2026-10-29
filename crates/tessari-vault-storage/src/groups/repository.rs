//! What the groups service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value};
use tessari_vault_types::{AccountId, CompanyId, GroupId, GroupKind, Name};

use super::entity::{GroupEntity, MemberRow};
use crate::Result;
use crate::group_keys::Revocation;

/// A department or team about to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewGroup {
    pub(crate) id: GroupId,
    pub(crate) kind: GroupKind,
    pub(crate) name: Name,
    pub(crate) parent: GroupId,
}

/// A member leaving a group: who, which groups' keys their leaving retires, and their wrap of the group's key.
pub(crate) struct Leaving<'a> {
    pub(crate) company: &'a CompanyId,
    pub(crate) group: &'a GroupId,
    pub(crate) account: &'a AccountId,
    pub(crate) affected: Vec<String>,
    pub(crate) revocation: Option<Revocation>,
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
    /// The groups whose keys a member leaving `group` could hold: the group, and the department it sits in; `None`
    /// when the group is not the company's.
    async fn retiring(
        &self,
        place: &Place,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Option<Vec<String>>>;
    /// Take `account` out of `group`, marking the keys of `affected` and every record shared with them for rotation,
    /// and taking the leaver's wrap off the group's key when there is one.
    async fn remove_member(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        leaving: Leaving<'_>,
    ) -> Result<()>;
}
