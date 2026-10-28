//! What the companies service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value};
use tessari_vault_types::{
    AccountId, CompanyId, CompanyState, Email, GroupId, MemberState, Name, Role,
};

use super::entity::{CompanyEntity, Membership, MembershipEntity};
use crate::Result;

/// The account a new membership is for: one that exists, or one created in the same transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Joining {
    Existing(AccountId),
    New {
        id: AccountId,
        email: Email,
        name: Name,
    },
}

impl Joining {
    pub(crate) const fn id(&self) -> &AccountId {
        match self {
            Self::Existing(id) | Self::New { id, .. } => id,
        }
    }
}

/// A membership change, written whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Change {
    pub(crate) role: Role,
    pub(crate) state: MemberState,
    /// Whether the change takes an active owner away, so the company must keep another.
    pub(crate) loses_owner: bool,
}

#[async_trait]
pub(crate) trait CompanyRepository: Send + Sync {
    async fn create(
        &self,
        place: &Place,
        company: &CompanyId,
        name: &Name,
        owner: &Joining,
    ) -> Result<()>;
    async fn all(&self, place: &Place) -> Result<Vec<CompanyEntity>>;
    async fn by_ids(
        &self,
        place: &Place,
        ids: &[CompanyId],
        only_open: bool,
    ) -> Result<Vec<CompanyEntity>>;
    async fn memberships_of(
        &self,
        place: &Place,
        account: &AccountId,
    ) -> Result<Vec<MembershipEntity>>;
    async fn set_state(
        &self,
        place: &Place,
        company: &CompanyId,
        state: CompanyState,
    ) -> Result<()>;
    async fn members(&self, place: &Place, company: &CompanyId) -> Result<Vec<MembershipEntity>>;
    async fn membership(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<MembershipEntity>>;
    async fn add_member(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        joining: &Joining,
        role: Role,
    ) -> Result<()>;
    async fn change_member(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        target: &Membership,
        change: Change,
    ) -> Result<()>;
    async fn remove_member(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        target: &Membership,
        groups: &[GroupId],
        loses_owner: bool,
    ) -> Result<()>;
    /// The groups `account` is in, with the company each belongs to.
    async fn groups_of(&self, place: &Place, account: &AccountId) -> Result<Vec<(String, String)>>;
}
