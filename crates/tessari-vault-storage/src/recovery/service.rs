//! The company recovery key (C7). An owner keys it once, wrapped only to owners' key sets, because whoever holds it
//! reaches every member's escrowed keys; every member reads its public half, and each owner only their own wrap.

use std::collections::HashSet;
use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{CompanyId, KeyId};

use super::entity::RecoveryKey;
use super::repository::{RecoveryRepository, Retiring};
use crate::access::{self, Actor, recorded};
use crate::group_keys::{MemberWrap, NewGroupKey};
use crate::keys::PublicKeys;
use crate::records::RecordVaults;
use crate::{Conflict, Error, Result};

/// The company recovery key.
pub struct RecoveryService {
    pub(super) repository: Arc<dyn RecoveryRepository>,
    pub(super) vaults: Arc<RecordVaults>,
    pub(super) database: Database,
    pub(super) place: Place,
}

impl RecoveryService {
    pub(crate) fn new(
        repository: Arc<dyn RecoveryRepository>,
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

    /// Keep the company's recovery key `key`, wrapped to owners' key sets; a key due for rotation is replaced, and the
    /// escrows made to it are dropped, so each member escrows again to the new one.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only); [`Conflict::RecoveryKeyed`] when the company has one not
    /// due for rotation;
    /// [`Conflict::OutsideReach`] for no wrap, a wrap to a key set that is not an owner's, or two to one.
    pub async fn keep(&self, actor: &Actor, company: &CompanyId, key: NewGroupKey) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        let existing = self.repository.get(&self.place, company).await?;
        if existing.as_ref().is_some_and(|entity| !entity.rotate) {
            return Err(Error::Conflict(Conflict::RecoveryKeyed));
        }
        let owners: HashSet<String> = self
            .repository
            .owners(&self.place, company)
            .await?
            .into_iter()
            .map(|owner| owner.key_id)
            .collect();
        let mut seen = HashSet::new();
        let within = !key.wraps().is_empty()
            && key.wraps().iter().all(|wrap| {
                let id = wrap.key_id().as_str();
                owners.contains(id) && seen.insert(id)
            });
        if !within {
            return Err(Error::Conflict(Conflict::OutsideReach));
        }
        let vault_key = self.vaults.key(company, standing.key_epoch)?;
        let Some(retiring) = existing else {
            return self
                .repository
                .keep(
                    &self.place,
                    recorded(
                        actor,
                        company,
                        &standing,
                        "recovery.key",
                        key.key_id().as_str(),
                    ),
                    company,
                    &key,
                    vault_key,
                )
                .await;
        };
        let escrowed = self
            .repository
            .escrowed_to(&self.place, company, &retiring.key_id)
            .await?;
        self.repository
            .rekey(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "recovery.key.rotate",
                    key.key_id().as_str(),
                ),
                company,
                Retiring {
                    key_id: &retiring.key_id,
                    escrowed: &escrowed,
                },
                &key,
                vault_key,
            )
            .await
    }

    /// The active owners' key sets that hold no wrap of the recovery key yet.
    async fn open(&self, company: &CompanyId, key_epoch: u32) -> Result<Vec<PublicKeys>> {
        if self.repository.get(&self.place, company).await?.is_none() {
            return Err(Error::NotFound);
        }
        let wrapped: HashSet<String> = self
            .repository
            .wraps(
                &self.place,
                None,
                company,
                self.vaults.key(company, key_epoch)?,
            )
            .await?
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        self.repository
            .owners(&self.place, company)
            .await?
            .into_iter()
            .filter(|key| !wrapped.contains(&key.key_id))
            .map(crate::keys::KeyEntity::into_public)
            .collect()
    }

    /// The owners' key sets still without a wrap of the recovery key, for an owner who holds one to wrap it to.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only); [`Error::NotFound`] when no owner has made one.
    pub async fn pending(&self, actor: &Actor, company: &CompanyId) -> Result<Vec<PublicKeys>> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        self.open(company, standing.key_epoch).await
    }

    /// Add `wraps` of the recovery key, each to an owner's key set that holds none yet.
    ///
    /// # Errors
    ///
    /// As [`RecoveryService::pending`], and [`Conflict::OutsideReach`] for no wrap, a wrap to a key set that is not
    /// a waiting owner's, or two to one.
    pub async fn add_wraps(
        &self,
        actor: &Actor,
        company: &CompanyId,
        wraps: Vec<MemberWrap>,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        let open: HashSet<String> = self
            .open(company, standing.key_epoch)
            .await?
            .into_iter()
            .map(|key| key.key_id.as_str().to_owned())
            .collect();
        let mut seen = HashSet::new();
        let within = !wraps.is_empty()
            && wraps.iter().all(|wrap| {
                let id = wrap.key_id().as_str();
                open.contains(id) && seen.insert(id)
            });
        if !within {
            return Err(Error::Conflict(Conflict::OutsideReach));
        }
        self.repository
            .add_wraps(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "recovery.key.wrap",
                    company.as_str(),
                ),
                company,
                &wraps,
                self.vaults.key(company, standing.key_epoch)?,
            )
            .await
    }

    /// The company's recovery key, for any member: what a browser escrows to once the chain vouches for it.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] when no owner has made one.
    pub async fn read(&self, actor: &Actor, company: &CompanyId) -> Result<RecoveryKey> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadKeys,
        )
        .await?;
        self.repository
            .get(&self.place, company)
            .await?
            .ok_or(Error::NotFound)?
            .into_model()
    }

    /// The calling owner's own wrap of the recovery key; the read is on the company's trail.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only); [`Error::NotFound`] when there is no key or no wrap to
    /// the caller's key set.
    pub async fn mine(&self, actor: &Actor, company: &CompanyId) -> Result<(KeyId, Vec<u8>)> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeyRecovery,
        )
        .await?;
        let key = self
            .repository
            .get(&self.place, company)
            .await?
            .ok_or(Error::NotFound)?
            .into_model()?;
        let own = self
            .repository
            .own_key(&self.place, company, actor.account())
            .await?
            .ok_or(Error::NotFound)?;
        let wrap = self
            .repository
            .wraps(
                &self.place,
                Some(recorded(
                    actor,
                    company,
                    &standing,
                    "recovery.key.read",
                    key.key_id.as_str(),
                )),
                company,
                self.vaults.key(company, standing.key_epoch)?,
            )
            .await?
            .into_iter()
            .find_map(|(id, wrap)| (id == own).then_some(wrap))
            .ok_or(Error::NotFound)?;
        Ok((key.key_id, wrap))
    }
}
