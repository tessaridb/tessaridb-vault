//! The storage aggregate: every domain's service, assembled in one place.

use std::sync::Arc;

use tessari_vault_infrastructure::tessaridb::{Database, Place};

use crate::accounts::{AccountService, TessaridbAccountRepository};
use crate::companies::{CompanyService, TessaridbCompanyRepository};
use crate::groups::{GroupService, TessaridbGroupRepository};
use crate::sessions::SessionService;
use crate::throttle::ThrottleService;

/// The services the server reaches its records through.
#[derive(Clone)]
pub struct Storage {
    /// Accounts.
    pub accounts: Arc<AccountService>,
    /// Companies and memberships.
    pub companies: Arc<CompanyService>,
    /// Departments and teams.
    pub groups: Arc<GroupService>,
    /// Sessions.
    pub sessions: Arc<SessionService>,
    /// Request counters.
    pub throttle: Arc<ThrottleService>,
}

impl Storage {
    /// Every service, over `database`, working at `place`.
    #[must_use]
    pub fn new(database: &Database, place: &Place) -> Self {
        let accounts = Arc::new(AccountService::new(
            Arc::new(TessaridbAccountRepository::new(database.clone())),
            place.clone(),
        ));
        let companies = Arc::new(CompanyService::new(
            Arc::new(TessaridbCompanyRepository::new(database.clone())),
            Arc::clone(&accounts),
            database.clone(),
            place.clone(),
        ));
        let groups = Arc::new(GroupService::new(
            Arc::new(TessaridbGroupRepository::new(database.clone())),
            Arc::clone(&accounts),
            database.clone(),
            place.clone(),
        ));
        let sessions = Arc::new(SessionService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&accounts),
        ));
        let throttle = Arc::new(ThrottleService::new(database.clone(), place.clone()));
        Self {
            accounts,
            companies,
            groups,
            sessions,
            throttle,
        }
    }
}
