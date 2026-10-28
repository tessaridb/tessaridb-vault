//! Companies and their members. Every call takes the caller and decides before it reads or writes anything a
//! caller outside the company could learn from.

use std::collections::BTreeMap;
use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{
    AccountId, CompanyId, CompanyState, Email, GroupId, MemberState, Name, Role,
};

use super::entity::{Company, CompanyView, Member, Membership};
use super::repository::{Change, CompanyRepository, Joining};
use crate::access::{self, Actor, guard_values};
use crate::accounts::AccountService;
use crate::{Error, Result};

/// Companies and memberships.
pub struct CompanyService {
    repository: Arc<dyn CompanyRepository>,
    accounts: Arc<AccountService>,
    database: Database,
    place: Place,
}

impl CompanyService {
    pub(crate) fn new(
        repository: Arc<dyn CompanyRepository>,
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

    /// Create a company with its first owner (R1), answering the company and the owner's account. Platform admins
    /// only.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone else; [`Error::Store`] when the store fails.
    pub async fn create(
        &self,
        actor: &Actor,
        name: &Name,
        owner_email: &Email,
        owner_name: &Name,
    ) -> Result<(Company, AccountId)> {
        actor.platform()?;
        let joining = self.joining(owner_email, owner_name).await?;
        let id = CompanyId::from_random(random::bytes()?);
        self.repository
            .create(&self.place, &id, name, &joining)
            .await?;
        let owner = joining.id().clone();
        Ok((
            Company {
                id,
                name: name.clone(),
                state: CompanyState::Active,
            },
            owner,
        ))
    }

    /// The companies `actor` may see: every company for a platform admin, otherwise the open companies they are an
    /// active member of, each with the caller's role.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn list(&self, actor: &Actor) -> Result<Vec<CompanyView>> {
        let memberships = self
            .repository
            .memberships_of(&self.place, actor.account())
            .await?;
        let mut roles = BTreeMap::new();
        for entity in memberships {
            let held = entity.into_model()?;
            if held.state == MemberState::Active {
                roles.insert(held.company, held.role);
            }
        }
        let companies = if actor.is_platform_admin() {
            self.repository.all(&self.place).await?
        } else {
            let ids: Vec<CompanyId> = roles.keys().cloned().collect();
            self.repository.by_ids(&self.place, &ids, true).await?
        };
        companies
            .into_iter()
            .map(|entity| {
                let company = entity.into_model()?;
                let role = roles.get(&company.id).copied();
                Ok(CompanyView { company, role })
            })
            .collect()
    }

    /// Suspend or resume a company (R1). Platform admins only.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone else; [`Error::NotFound`] for no such company.
    pub async fn set_state(
        &self,
        actor: &Actor,
        company: &CompanyId,
        state: CompanyState,
    ) -> Result<()> {
        actor.platform()?;
        self.repository.set_state(&self.place, company, state).await
    }

    /// The company's members (R2, read).
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member of the open company.
    pub async fn members(&self, actor: &Actor, company: &CompanyId) -> Result<Vec<Member>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadMembers,
        )
        .await?;
        let memberships = self.repository.members(&self.place, company).await?;
        let held: Vec<Membership> = memberships
            .into_iter()
            .map(super::entity::MembershipEntity::into_model)
            .collect::<Result<_>>()?;
        let ids: Vec<AccountId> = held
            .iter()
            .map(|membership| membership.account.clone())
            .collect();
        let mut accounts: BTreeMap<AccountId, _> = self
            .accounts
            .many(&ids)
            .await?
            .into_iter()
            .map(|account| (account.id.clone(), account))
            .collect();
        held.into_iter()
            .map(|membership| {
                let account = accounts
                    .remove(&membership.account)
                    .ok_or(Error::Unreadable {
                        record: "membership",
                        reason: "a membership whose account is gone".to_owned(),
                    })?;
                Ok(Member {
                    account,
                    role: membership.role,
                    state: membership.state,
                })
            })
            .collect()
    }

    /// Add the person with `email` to the company as `role`, creating their account when the address is new (R2).
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::Conflict`] when they are already a member.
    pub async fn add_member(
        &self,
        actor: &Actor,
        company: &CompanyId,
        email: &Email,
        name: &Name,
        role: Role,
    ) -> Result<AccountId> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::AddMember { role },
        )
        .await?;
        let joining = self.joining(email, name).await?;
        let account = joining.id().clone();
        self.repository
            .add_member(
                &self.place,
                guard_values(actor, company, &standing),
                company,
                &joining,
                role,
            )
            .await?;
        Ok(account)
    }

    /// Change a member's role, state or both (R2). Making or unmaking an owner is an owner's act, and a company
    /// keeps at least one active owner.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] when the account is not a member;
    /// [`Error::Conflict`] for the last owner.
    pub async fn change_member(
        &self,
        actor: &Actor,
        company: &CompanyId,
        account: &AccountId,
        role: Option<Role>,
        state: Option<MemberState>,
    ) -> Result<()> {
        let target = self.visible_target(actor, company, account).await?;
        let to_role = role.unwrap_or(target.role);
        let to_state = state.unwrap_or(target.state);
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ChangeMember {
                from: target.role,
                to: to_role,
            },
        )
        .await?;
        let loses_owner = owns(target.role, target.state) && !owns(to_role, to_state);
        let change = Change {
            role: to_role,
            state: to_state,
            loses_owner,
        };
        self.repository
            .change_member(
                &self.place,
                guard_values(actor, company, &standing),
                &target,
                change,
            )
            .await
    }

    /// Remove a member from the company and from every group of it (R2).
    ///
    /// # Errors
    ///
    /// As [`CompanyService::change_member`].
    pub async fn remove_member(
        &self,
        actor: &Actor,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<()> {
        let target = self.visible_target(actor, company, account).await?;
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::RemoveMember { role: target.role },
        )
        .await?;
        let groups: Vec<GroupId> = self
            .repository
            .groups_of(&self.place, account)
            .await?
            .into_iter()
            .filter(|(_, owner)| owner == company.as_str())
            .map(|(group, _)| GroupId::parse(&group))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::Unreadable {
                record: "group",
                reason: "a group id did not read back as written".to_owned(),
            })?;
        let loses_owner = owns(target.role, target.state);
        self.repository
            .remove_member(
                &self.place,
                guard_values(actor, company, &standing),
                &target,
                &groups,
                loses_owner,
            )
            .await
    }

    /// The membership of `account` in `company`, after the caller has been shown the company at all, so a caller
    /// outside it learns nothing about who is inside.
    async fn visible_target(
        &self,
        actor: &Actor,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Membership> {
        let standing = access::standing(&self.database, &self.place, actor, company).await?;
        access::require(standing.as_ref(), Action::ReadMembers)?;
        self.repository
            .membership(&self.place, company, account)
            .await?
            .ok_or(Error::NotFound)?
            .into_model()
    }

    async fn joining(&self, email: &Email, name: &Name) -> Result<Joining> {
        Ok(match self.accounts.find_by_email(email).await? {
            Some(account) => Joining::Existing(account.id),
            None => Joining::New {
                id: AccountId::from_random(random::bytes()?),
                email: email.clone(),
                name: name.clone(),
            },
        })
    }
}

/// Whether a membership in this role and state is an active owner.
fn owns(role: Role, state: MemberState) -> bool {
    role == Role::Owner && state == MemberState::Active
}
