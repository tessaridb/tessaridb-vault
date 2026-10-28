//! Departments and teams (R3). Every call decides first, so a caller who may not see the company or may not manage
//! it learns nothing about which groups exist.

use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{AccountId, CompanyId, GroupId, GroupKind, Name};

use super::entity::{Group, GroupEntity};
use super::repository::{GroupRepository, NewGroup};
use crate::access::{self, Actor, recorded};
use crate::accounts::{Account, AccountService};
use crate::{Conflict, Error, Result};

/// Departments and teams.
pub struct GroupService {
    repository: Arc<dyn GroupRepository>,
    accounts: Arc<AccountService>,
    database: Database,
    place: Place,
}

impl GroupService {
    pub(crate) fn new(
        repository: Arc<dyn GroupRepository>,
        accounts: Arc<AccountService>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            accounts,
            database,
            place,
        }
    }

    /// The company's departments and teams.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member of the open company.
    pub async fn list(&self, actor: &Actor, company: &CompanyId) -> Result<Vec<Group>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadGroups,
        )
        .await?;
        self.repository
            .list(&self.place, company)
            .await?
            .into_iter()
            .map(GroupEntity::into_model)
            .collect()
    }

    /// Who is in one department or team.
    ///
    /// # Errors
    ///
    /// As [`GroupService::list`], and [`Error::NotFound`] for a group that is not the company's.
    pub async fn members(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Vec<Account>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadGroups,
        )
        .await?;
        self.repository
            .get(&self.place, company, group)
            .await?
            .ok_or(Error::NotFound)?;
        let ids = self
            .repository
            .members(&self.place, group)
            .await?
            .into_iter()
            .map(super::entity::MemberRow::into_id)
            .collect::<Result<Vec<AccountId>>>()?;
        self.accounts.many(&ids).await
    }

    /// Create a department in the company, or a team in the company or in one of its departments.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::Conflict`] for a parent the kind cannot sit in.
    pub async fn create(
        &self,
        actor: &Actor,
        company: &CompanyId,
        kind: GroupKind,
        name: &Name,
        parent: Option<&GroupId>,
    ) -> Result<Group> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::CreateGroup,
        )
        .await?;
        let parent = match (kind, parent) {
            (GroupKind::Company, _) | (GroupKind::Department, Some(_)) => {
                return Err(Error::Conflict(Conflict::WrongParent));
            }
            (GroupKind::Department | GroupKind::Team, None) => company.group(),
            (GroupKind::Team, Some(department)) => department.clone(),
        };
        let group = NewGroup {
            id: GroupId::from_random(random::bytes()?),
            kind,
            name: name.clone(),
            parent,
        };
        self.repository
            .create(
                &self.place,
                recorded(actor, company, &standing, "group.create", group.id.as_str()),
                company,
                &group,
            )
            .await?;
        Ok(Group {
            id: group.id,
            kind: group.kind,
            name: group.name,
            parent: group.parent,
        })
    }

    /// Rename a department or team.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a group that is not the company's.
    pub async fn rename(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
        name: &Name,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::RenameGroup,
        )
        .await?;
        self.repository
            .rename(
                &self.place,
                recorded(actor, company, &standing, "group.rename", group.as_str()),
                company,
                group,
                name,
            )
            .await
    }

    /// Delete a department or team that holds no other group; its members leave it.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`]; [`Error::Conflict`] when groups sit inside it.
    pub async fn delete(&self, actor: &Actor, company: &CompanyId, group: &GroupId) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::DeleteGroup,
        )
        .await?;
        let found = self
            .repository
            .get(&self.place, company, group)
            .await?
            .ok_or(Error::NotFound)?;
        let members = self
            .repository
            .members(&self.place, group)
            .await?
            .into_iter()
            .map(super::entity::MemberRow::into_id)
            .collect::<Result<Vec<AccountId>>>()?;
        self.repository
            .delete(
                &self.place,
                recorded(actor, company, &standing, "group.delete", group.as_str()),
                company,
                &found,
                &members,
            )
            .await
    }

    /// Put an active member of the company into one of its groups.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`]; [`Error::Conflict`] when the account is not an
    /// active member.
    pub async fn add_member(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
        account: &AccountId,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::AddToGroup,
        )
        .await?;
        self.repository
            .add_member(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "group.member.add",
                    &format!("{group}:{account}"),
                ),
                company,
                group,
                account,
            )
            .await
    }

    /// Take a member out of one of the company's groups.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a group that is not the company's.
    pub async fn remove_member(
        &self,
        actor: &Actor,
        company: &CompanyId,
        group: &GroupId,
        account: &AccountId,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::RemoveFromGroup,
        )
        .await?;
        self.repository
            .remove_member(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "group.member.remove",
                    &format!("{group}:{account}"),
                ),
                company,
                group,
                account,
            )
            .await
    }
}
