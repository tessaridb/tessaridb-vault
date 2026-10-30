//! The storage aggregate: every domain's service, assembled in one place.

use std::sync::Arc;

use tessari_vault_infrastructure::custody::{Passwords, SigninKeys, TotpSeal, VaultKeys};
use tessari_vault_infrastructure::s3::ObjectStorage;
use tessari_vault_infrastructure::tessaridb::{Database, Place};

use crate::accounts::{AccountService, TessaridbAccountRepository};
use crate::audit::AuditService;
use crate::chain::{ChainService, TessaridbChainRepository};
use crate::companies::{CompanyService, TessaridbCompanyRepository};
use crate::company_drops::CompanyDropService;
use crate::enrolments::EnrolmentService;
use crate::files::{FileService, S3Files, TessaridbFiles};
use crate::group_keys::{GroupKeyService, TessaridbGroupKeyRepository};
use crate::groups::{GroupService, TessaridbGroupRepository};
use crate::identities::IdentityService;
use crate::keys::{KeyService, TessaridbKeyRepository};
use crate::passkeys::PasskeyService;
use crate::record_types::{RecordTypeService, TessaridbRecordTypeRepository};
use crate::records::{RecordService, RecordVaults, TessaridbRecordRepository};
use crate::recovery::{RecoveryService, TessaridbRecoveryRepository};
use crate::recovery_requests::{RecoveryRequestService, TessaridbRecoveryRequestRepository};
use crate::rotation::RotationService;
use crate::sessions::SessionService;
use crate::shares::{Reach, ShareService, TessaridbShareRepository};
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
    /// The keys that open each company's records vault.
    pub vault_keys: Arc<VaultKeys>,
}

/// The services the server reaches its records through.
#[derive(Clone)]
pub struct Storage {
    /// Accounts.
    pub accounts: Arc<AccountService>,
    /// The company audit trails.
    pub audit: Arc<AuditService>,
    /// The companies' vouching chains.
    pub chain: Arc<ChainService>,
    /// Companies and memberships.
    pub companies: Arc<CompanyService>,
    /// Deleting a company: an owner's signed request, the platform admin's confirmation, a waiting period.
    pub company_drops: Arc<CompanyDropService>,
    /// Enrolment links and the credentials they create.
    pub enrolments: Arc<EnrolmentService>,
    /// Encrypted files on records.
    pub files: Arc<FileService>,
    /// Group keys and their wraps.
    pub group_keys: Arc<GroupKeyService>,
    /// Departments and teams.
    pub groups: Arc<GroupService>,
    /// Identities at an OpenID Connect issuer, and requests in flight to one.
    pub identities: Arc<IdentityService>,
    /// Members' key sets.
    pub keys: Arc<KeyService>,
    /// The company recovery keys.
    pub recovery: Arc<RecoveryService>,
    /// Recovery requests and their handoffs.
    pub recovery_requests: Arc<RecoveryRequestService>,
    /// Moving every company vault to the current service-key epoch, run by the operator.
    pub rotation: Arc<RotationService>,
    /// Passkeys and their ceremonies.
    pub passkeys: Arc<PasskeyService>,
    /// Record types the company defined.
    pub record_types: Arc<RecordTypeService>,
    /// Records and their revisions.
    pub records: Arc<RecordService>,
    /// Sessions.
    pub sessions: Arc<SessionService>,
    /// Records shared with groups and members.
    pub shares: Arc<ShareService>,
    /// Own sign-in.
    pub sign_in: Arc<SignInService>,
    /// Request counters.
    pub throttle: Arc<ThrottleService>,
}

impl Storage {
    /// Every service, over `database`, working at `place`, with the keys the service key made.
    #[must_use]
    pub fn new(
        database: &Database,
        place: &Place,
        custody: Custody,
        objects: Option<Arc<dyn ObjectStorage>>,
    ) -> Self {
        let Custody {
            passwords,
            seal,
            signin_keys,
            vault_keys,
        } = custody;
        let rotation = Arc::new(RotationService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&vault_keys),
            Arc::clone(&seal),
        ));
        let company_drops = Arc::new(CompanyDropService::new(
            database.clone(),
            place.clone(),
            objects.clone(),
        ));
        let vaults = Arc::new(RecordVaults::new(database.clone(), vault_keys));
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
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
        ));
        let group_keys = Arc::new(GroupKeyService::new(
            Arc::new(TessaridbGroupKeyRepository::new(database.clone())),
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
        ));
        let recovery = Arc::new(RecoveryService::new(
            Arc::new(TessaridbRecoveryRepository::new(database.clone())),
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
        ));
        let recovery_requests = Arc::new(RecoveryRequestService::new(
            Arc::new(TessaridbRecoveryRequestRepository::new(database.clone())),
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
        ));
        let groups = Arc::new(GroupService::new(
            Arc::new(TessaridbGroupRepository::new(database.clone())),
            Arc::clone(&accounts),
            Arc::clone(&group_keys),
            database.clone(),
            place.clone(),
        ));
        let identities = Arc::new(IdentityService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&accounts),
            Arc::clone(&sessions),
        ));
        let passkeys = Arc::new(PasskeyService::new(
            database.clone(),
            place.clone(),
            Arc::clone(&accounts),
            Arc::clone(&sessions),
        ));
        let record_types = Arc::new(RecordTypeService::new(
            Arc::new(TessaridbRecordTypeRepository::new(database.clone())),
            database.clone(),
            place.clone(),
        ));
        let share_repository = Arc::new(TessaridbShareRepository::new(database.clone()));
        let records = Arc::new(RecordService::new(
            Arc::new(TessaridbRecordRepository::new(database.clone())),
            Arc::clone(&vaults),
            Arc::clone(&record_types),
            Arc::new(Reach::new(share_repository.clone(), place.clone())),
            database.clone(),
            place.clone(),
        ));
        let shares = Arc::new(ShareService::new(
            share_repository,
            Arc::clone(&records),
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
        ));
        let repository: Arc<dyn crate::files::FileRepository> = match objects {
            Some(ref client) => Arc::new(S3Files::new(database.clone(), Arc::clone(client))),
            None => Arc::new(TessaridbFiles::new(database.clone())),
        };
        let files = Arc::new(FileService::new(
            repository,
            Arc::clone(&records),
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
            objects,
        ));
        let keys = Arc::new(KeyService::new(
            Arc::new(TessaridbKeyRepository::new(database.clone())),
            Arc::clone(&vaults),
            database.clone(),
            place.clone(),
        ));
        let chain = Arc::new(ChainService::new(
            Arc::new(TessaridbChainRepository::new(database.clone())),
            database.clone(),
            place.clone(),
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
            audit: Arc::new(AuditService::new(database.clone(), place.clone())),
            chain,
            companies,
            company_drops,
            enrolments,
            files,
            group_keys,
            groups,
            identities,
            keys,
            passkeys,
            recovery,
            recovery_requests,
            rotation,
            record_types,
            records,
            shares,
            sessions,
            sign_in,
            throttle,
        }
    }
}
