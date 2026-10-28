//! The storage aggregate: every domain's service, assembled in one place.

use std::sync::Arc;

use tessari_vault_infrastructure::custody::{Passwords, SigninKeys, TotpSeal};
use tessari_vault_infrastructure::tessaridb::{Database, Place};

use crate::accounts::{AccountService, TessaridbAccountRepository};
use crate::companies::{CompanyService, TessaridbCompanyRepository};
use crate::enrolments::EnrolmentService;
use crate::groups::{GroupService, TessaridbGroupRepository};
use crate::identities::IdentityService;
use crate::sessions::SessionService;
use crate::signin::{SignInParts, SignInService};
use crate::throttle::ThrottleService;

/// The keys the service key made, which the storage services sign people in and enrol them with.
pub struct Custody {
    /// The sign-in password verifier.
    pub passwords: Arc<Passwords>,
    /// The seal TOTP seeds are kept under.
    pub seal: Arc<TotpSeal>,
    /// The names sign-in attempts are counted under.
    pub signin_keys: SigninKeys,
}

/// The services the server reaches its records through.
#[derive(Clone)]
pub struct Storage {
    /// Accounts.
    pub accounts: Arc<AccountService>,
    /// Companies and memberships.
    pub companies: Arc<CompanyService>,
    /// Enrolment links and the credentials they create.
    pub enrolments: Arc<EnrolmentService>,
    /// Departments and teams.
    pub groups: Arc<GroupService>,
    /// Identities at an OpenID Connect issuer, and requests in flight to one.
    pub identities: Arc<IdentityService>,
    /// Sessions.
    pub sessions: Arc<SessionService>,
    /// Own sign-in.
    pub sign_in: Arc<SignInService>,
    /// Request counters.
    pub throttle: Arc<ThrottleService>,
}

impl Storage {
    /// Every service, over `database`, working at `place`, with the keys the service key made.
    #[must_use]
    pub fn new(database: &Database, place: &Place, custody: Custody) -> Self {
        let Custody {
            passwords,
            seal,
            signin_keys,
        } = custody;
        let accounts = Arc::new(AccountService::new(
            Arc::new(TessaridbAccountRepository::new(database.clone())),
            place.clone(),
        ));
        let sessions = Arc::new(SessionService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&accounts),
        ));
        let enrolments = Arc::new(EnrolmentService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&accounts),
            Arc::clone(&passwords),
            Arc::clone(&seal),
            Arc::clone(&sessions),
        ));
        let companies = Arc::new(CompanyService::new(
            Arc::new(TessaridbCompanyRepository::new(database.clone())),
            Arc::clone(&accounts),
            Arc::clone(&enrolments),
            database.clone(),
            place.clone(),
        ));
        let groups = Arc::new(GroupService::new(
            Arc::new(TessaridbGroupRepository::new(database.clone())),
            Arc::clone(&accounts),
            database.clone(),
            place.clone(),
        ));
        let identities = Arc::new(IdentityService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&accounts),
            Arc::clone(&sessions),
        ));
        let throttle = Arc::new(ThrottleService::new(database.clone(), place.clone()));
        let sign_in = Arc::new(SignInService::new(
            database.clone(),
            place.clone(),
            SignInParts {
                accounts: Arc::clone(&accounts),
                passwords,
                seal,
                keys: signin_keys,
                sessions: Arc::clone(&sessions),
                throttle: Arc::clone(&throttle),
            },
        ));
        Self {
            accounts,
            companies,
            enrolments,
            groups,
            identities,
            sessions,
            sign_in,
            throttle,
        }
    }
}
