//! Recovery requests (C7). The member asks; only an owner sees open requests and hands one over; only the member who
//! asked redeems it, once, and then wraps their keys under a new unlock key, once. A second redemption is refused and
//! written to the company's trail, because it means the handoff was delivered twice or taken by somebody else.

use std::sync::Arc;

use tessari_vault_constants::RECOVERY_REQUEST_SECONDS;
use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{CompanyId, KeyId};

use super::entity::{OpenRequest, RequestEntity};
use super::repository::{Asking, RecoveryRequestRepository, Rewrapped};
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;
use crate::keys::Kdf;
use crate::records::RecordVaults;
use crate::{Conflict, Error, Result};

/// Recovery requests.
pub struct RecoveryRequestService {
    repository: Arc<dyn RecoveryRequestRepository>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
}

impl RecoveryRequestService {
    pub(crate) fn new(
        repository: Arc<dyn RecoveryRequestRepository>,
        vaults: Arc<RecordVaults>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            vaults,
            database,
            place,
        }
    }

    /// Ask to be recovered with the one-time X25519 `handoff_public`; the request's id.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Conflict::NoEscrow`] when the caller's keys are not escrowed here.
    pub async fn ask(
        &self,
        actor: &Actor,
        company: &CompanyId,
        handoff_public: &[u8],
    ) -> Result<KeyId> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        let id = KeyId::from_random(random::bytes()?);
        let expires = epoch_seconds()
            .checked_add(RECOVERY_REQUEST_SECONDS)
            .ok_or(Error::Conflict(Conflict::RecoveryClosed))?;
        self.repository
            .open(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "member.recovery.request",
                    actor.account().as_str(),
                ),
                company,
                &id,
                Asking {
                    account: actor.account(),
                    handoff_public,
                    expires,
                },
            )
            .await?;
        Ok(id)
    }

    /// The company's requests no owner has handed over yet, for an owner.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only).
    pub async fn open_requests(
        &self,
        actor: &Actor,
        company: &CompanyId,
    ) -> Result<Vec<OpenRequest>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        self.repository
            .open_requests(&self.place, company, epoch_seconds())
            .await?
            .into_iter()
            .map(RequestEntity::into_model)
            .collect()
    }

    /// Keep the owner's handoff `wrap` for request `id`; the member is told.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only); [`Error::NotFound`] for a request that is not the
    /// company's; [`Conflict::RecoveryClosed`] when it was handed over already or has expired.
    pub async fn hand_over(
        &self,
        actor: &Actor,
        company: &CompanyId,
        id: &KeyId,
        wrap: &[u8],
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        self.repository
            .hand_over(
                &self.place,
                recorded(actor, company, &standing, "member.recover", id.as_str()),
                company,
                id,
                wrap,
                self.vaults.key(company)?,
            )
            .await
    }

    /// The handoff wrap of the caller's request `id`, once.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a request that is not the caller's;
    /// [`Conflict::NotHandedOver`] before an owner has handed it over; [`Conflict::RecoveryClosed`] when it was
    /// redeemed already or has expired, which is also written to the trail.
    pub async fn redeem(&self, actor: &Actor, company: &CompanyId, id: &KeyId) -> Result<Vec<u8>> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        let redeemed = self
            .repository
            .redeem(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "member.recovery.redeem",
                    id.as_str(),
                ),
                company,
                actor.account(),
                id,
                self.vaults.key(company)?,
            )
            .await;
        if let Err(Error::Conflict(Conflict::RecoveryClosed)) = redeemed {
            self.repository
                .record(
                    &self.place,
                    recorded(
                        actor,
                        company,
                        &standing,
                        "member.recovery.replay",
                        id.as_str(),
                    ),
                )
                .await?;
        }
        redeemed
    }

    /// Replace the caller's wrapped keys and unlock parameters, using up their redeemed request `request`.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a request that is not the caller's;
    /// [`Conflict::RecoveryClosed`] unless it was redeemed and not yet used.
    pub async fn rewrap(
        &self,
        actor: &Actor,
        company: &CompanyId,
        request: &KeyId,
        kdf: &Kdf,
        wrapped: &[u8],
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        self.repository
            .rewrap(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "keys.rewrap",
                    actor.account().as_str(),
                ),
                company,
                actor.account(),
                Rewrapped {
                    request,
                    kdf,
                    wrapped,
                },
                self.vaults.key(company)?,
            )
            .await
    }
}
