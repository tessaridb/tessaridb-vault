//! The company recovery key (C7). An owner keys it once, wrapped only to owners' key sets, because whoever holds it
//! reaches every member's escrowed keys; every member reads its public half, and each owner only their own wrap.

use std::collections::HashSet;
use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{CompanyId, KeyId};

use super::entity::RecoveryKey;
use super::repository::RecoveryRepository;
use crate::access::{self, Actor, recorded};
use crate::group_keys::NewGroupKey;
use crate::records::RecordVaults;
use crate::{Conflict, Error, Result};

/// The company recovery key.
pub struct RecoveryService {
    repository: Arc<dyn RecoveryRepository>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
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

    /// Keep the company's recovery key `key`, wrapped to owners' key sets.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (owners only); [`Conflict::RecoveryKeyed`] when the company has one;
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
        if self.repository.get(&self.place, company).await?.is_some() {
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
        self.repository
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
                self.vaults.key(company)?,
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
                recorded(
                    actor,
                    company,
                    &standing,
                    "recovery.key.read",
                    key.key_id.as_str(),
                ),
                company,
                self.vaults.key(company)?,
            )
            .await?
            .into_iter()
            .find_map(|(id, wrap)| (id == own).then_some(wrap))
            .ok_or(Error::NotFound)?;
        Ok((key.key_id, wrap))
    }
}
