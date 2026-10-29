//! Member key sets (KH-L1). A member keeps one set per company and reads it back; every member reads the public
//! halves of the company's sets, which is what a browser wraps record and group keys to.

use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::CompanyId;

use super::entity::{KeyEntity, KeySet, NewKeys, PublicKeys};
use super::repository::KeyRepository;
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;
use crate::records::RecordVaults;
use crate::{Error, Result};

/// Key sets.
pub struct KeyService {
    repository: Arc<dyn KeyRepository>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
}

impl KeyService {
    pub(crate) fn new(
        repository: Arc<dyn KeyRepository>,
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

    /// Keep `new` as the caller's key set in the company.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member; [`crate::Conflict::KeysExist`] when they keep one
    /// already; [`crate::Conflict::KeyTaken`] when its key id is another member's.
    pub async fn keep(
        &self,
        actor: &Actor,
        company: &CompanyId,
        new: NewKeys,
    ) -> Result<PublicKeys> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::KeepKeys,
        )
        .await?;
        let set = KeySet::of(actor.account().clone(), new);
        self.repository
            .keep(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "keys.keep",
                    set.public.key_id.as_str(),
                ),
                company,
                &set,
                epoch_seconds(),
                self.vaults.key(company, standing.key_epoch)?,
            )
            .await?;
        Ok(set.public)
    }

    /// The caller's own key set, wrapped half included; the read is on the company's trail before it is opened.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member; [`Error::NotFound`] when they keep none.
    pub async fn mine(&self, actor: &Actor, company: &CompanyId) -> Result<KeySet> {
        // A read, so a member can still unlock and read while the company waits to be deleted.
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadKeys,
        )
        .await?;
        let entity = self
            .repository
            .get(&self.place, company, actor.account())
            .await?
            .ok_or(Error::NotFound)?;
        let kdf = entity.kdf()?;
        let public = entity.into_public()?;
        let wrapped = self
            .repository
            .open(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "keys.read",
                    public.key_id.as_str(),
                ),
                company,
                actor.account(),
                self.vaults.key(company, standing.key_epoch)?,
            )
            .await?;
        let new = NewKeys::new(
            public.key_id,
            public.agreement_public,
            public.signing_public,
            kdf,
            wrapped,
        )
        .map_err(|_| Error::Unreadable {
            record: "key set",
            reason: "the key set did not read back as written".to_owned(),
        })?;
        Ok(KeySet::of(public.account, new))
    }

    /// The public halves of every key set kept in the company.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member of the open company.
    pub async fn directory(&self, actor: &Actor, company: &CompanyId) -> Result<Vec<PublicKeys>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadKeys,
        )
        .await?;
        self.repository
            .list(&self.place, company)
            .await?
            .into_iter()
            .map(KeyEntity::into_public)
            .collect()
    }
}
