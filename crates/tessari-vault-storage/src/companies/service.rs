//! Companies and their members. Every call takes the caller and decides before it reads or writes anything a
//! caller outside the company could learn from.

use std::collections::HashMap;
use std::sync::Arc;

use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{AccountId, CompanyId, CompanyState, Email, MemberState, Name};

use super::entity::{Company, CompanyView};
use super::repository::{CompanyRepository, Joining};
use crate::access::Actor;
use crate::accounts::AccountService;
use crate::audit;
use crate::enrolments::{EnrolmentService, EnrolmentToken};
use crate::records::RecordVaults;
use crate::{Error, Result};

mod members;

/// Companies and memberships.
pub struct CompanyService {
    repository: Arc<dyn CompanyRepository>,
    accounts: Arc<AccountService>,
    enrolments: Arc<EnrolmentService>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
}

impl CompanyService {
    pub(crate) fn new(
        repository: Arc<dyn CompanyRepository>,
        accounts: Arc<AccountService>,
        enrolments: Arc<EnrolmentService>,
        vaults: Arc<RecordVaults>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            accounts,
            enrolments,
            vaults,
            database,
            place,
        }
    }

    /// Create a company with its first owner (R1), answering the company, the owner's account and, when the owner has
    /// no credential yet, their enrolment link (Q-VA-34): nobody else may issue an owner's link, so the company's
    /// bootstrap hands it once to the platform admin who made it, on the new company's trail. Platform admins only.
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
    ) -> Result<(Company, AccountId, Option<EnrolmentToken>)> {
        actor.platform()?;
        let joining = self.joining(owner_email, owner_name).await?;
        let id = CompanyId::from_random(random::bytes()?);
        let event = audit::Event {
            company: id.as_str(),
            action: "company.create",
            target: id.as_str(),
        };
        self.vaults.declare(&self.place, &id).await?;
        self.repository
            .create(
                &self.place,
                audit::binds(actor, &event),
                &id,
                name,
                &joining,
            )
            .await?;
        let owner = joining.id().clone();
        let issued = audit::Event {
            company: id.as_str(),
            action: "enrolment.issue",
            target: owner.as_str(),
        };
        let link = match self
            .enrolments
            .issue_recorded(&owner, audit::binds(actor, &issued))
            .await
        {
            Ok(token) => Some(token),
            Err(Error::Conflict(crate::Conflict::Enrolled)) => None,
            Err(error) => return Err(error),
        };
        Ok((
            Company {
                id,
                name: name.clone(),
                state: CompanyState::Active,
            },
            owner,
            link,
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
        let mut roles = HashMap::new();
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
        let event = audit::Event {
            company: company.as_str(),
            action: if state == CompanyState::Active {
                "company.resume"
            } else {
                "company.suspend"
            },
            target: company.as_str(),
        };
        self.repository
            .set_state(&self.place, audit::binds(actor, &event), company, state)
            .await
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
