//! A company's members: reading them, adding, changing and removing them, and the enrolment link a member without
//! a credential is sent. Every call decides before it reads or writes anything a caller outside the company could
//! learn from.

use std::collections::HashMap;

use tessari_vault_core::access::Action;
use tessari_vault_types::{AccountId, CompanyId, Email, GroupId, MemberState, Name, Role};

use super::CompanyService;
use crate::access::{self, Actor, recorded};
use crate::audit;
use crate::companies::entity::{Member, Membership};
use crate::companies::repository::Change;
use crate::enrolments::EnrolmentToken;
use crate::{Error, Result};

impl CompanyService {
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
            .map(crate::companies::entity::MembershipEntity::into_model)
            .collect::<Result<_>>()?;
        let ids: Vec<AccountId> = held
            .iter()
            .map(|membership| membership.account.clone())
            .collect();
        let mut accounts: HashMap<AccountId, _> = self
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
                recorded(
                    actor,
                    company,
                    &standing,
                    "member.add",
                    joining.id().as_str(),
                ),
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
                recorded(actor, company, &standing, "member.change", account.as_str()),
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
        let held: Vec<(String, String, String)> = self
            .repository
            .groups_of(&self.place, account)
            .await?
            .into_iter()
            .filter(|(_, owner, _)| owner == company.as_str())
            .collect();
        // The company group's key and every key of their groups and of the groups those sit in retire.
        let mut retired: Vec<String> =
            vec![company.as_str().to_owned(), account.as_str().to_owned()];
        for (group, _, parent) in &held {
            retired.extend([group.clone(), parent.clone()]);
        }
        retired.sort_unstable();
        retired.dedup();
        let groups: Vec<GroupId> = held
            .iter()
            .map(|(group, _, _)| GroupId::parse(group))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::Unreadable {
                record: "group",
                reason: "a group id did not read back as written".to_owned(),
            })?;
        let loses_owner = owns(target.role, target.state);
        self.repository
            .remove_member(
                &self.place,
                recorded(actor, company, &standing, "member.remove", account.as_str()),
                &target,
                &groups,
                loses_owner,
                retired,
            )
            .await
    }

    /// An enrolment link for `account`, a member of `company` with no credential yet. Issuing it is managing that
    /// member, so the decision is the one a change to them takes: admins for members and admins, owners for anyone.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] when the account is not a member the caller may see; [`Error::Refused`] per the decision;
    /// [`Error::Conflict`] when the account is already enrolled.
    pub async fn issue_enrolment(
        &self,
        actor: &Actor,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<EnrolmentToken> {
        let target = self.visible_target(actor, company, account).await?;
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ChangeMember {
                from: target.role,
                to: target.role,
            },
        )
        .await?;
        self.enrolments
            .issue_recorded(
                account,
                audit::binds(
                    actor,
                    &audit::Event {
                        company: company.as_str(),
                        action: "enrolment.issue",
                        target: account.as_str(),
                    },
                ),
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
}

/// Whether a membership in this role and state is an active owner.
fn owns(role: Role, state: MemberState) -> bool {
    role == Role::Owner && state == MemberState::Active
}
