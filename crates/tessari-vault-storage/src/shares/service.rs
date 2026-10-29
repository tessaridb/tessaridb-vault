//! Sharing a record (C4). The caller must reach the record with edit access; the share row and the record key's
//! wrap to the target land in one transaction, which refuses a target outside the company, a key the target does
//! not hold, a second share to one target, and a record that moved since it was read.

use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place, RefusalClass};
use tessari_vault_types::{CompanyId, RecordId};

use super::entity::{NewShare, ShareTarget};
use super::repository::{ShareRemoval, ShareRepository, ShareWrite};
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;
use crate::records::{RecordService, RecordVaults};
use crate::{Conflict, Error, Result};

#[path = "listing.rs"]
mod listing;

/// Shares.
pub struct ShareService {
    repository: Arc<dyn ShareRepository>,
    records: Arc<RecordService>,
    vaults: Arc<RecordVaults>,
    database: Database,
    place: Place,
}

impl ShareService {
    pub(crate) fn new(
        repository: Arc<dyn ShareRepository>,
        records: Arc<RecordService>,
        vaults: Arc<RecordVaults>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            records,
            vaults,
            database,
            place,
        }
    }

    /// Share `record` as `share` says, wrapping its key on the latest revision to the target's key.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision and for a caller who only reads the record; [`Error::NotFound`] for a
    /// record the caller does not reach or a target outside the company; [`Conflict::NoKey`] for a target holding
    /// no key; [`Conflict::UnknownKey`] for a key that is not the target's; [`Conflict::AlreadyShared`] for a target
    /// the record is shared with, or its author; [`Conflict::StaleRevision`] or [`Conflict::StaleShares`] when the
    /// record moved since it was read.
    pub async fn share(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        share: &NewShare,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ShareRecord,
        )
        .await?;
        let (entity, reached) = self.records.reached(actor, company, record).await?;
        reached.editing()?;
        // The author opens the record under their own key already.
        if matches!(&share.target, ShareTarget::Member(account) if account.as_str() == entity.author)
        {
            return Err(Error::Conflict(Conflict::AlreadyShared));
        }
        let written = self
            .repository
            .create(
                &self.place,
                recorded(actor, company, &standing, "record.share", record.as_str()),
                ShareWrite {
                    company,
                    record,
                    revision: entity.revision,
                    share_version: entity.share_version,
                    share,
                    by: actor.account(),
                    now: epoch_seconds(),
                    vault_key: self.vaults.key(company)?,
                },
            )
            .await;
        match written {
            // Two shares or a share and a revision at once: the store reports the loser as contention.
            Err(Error::Store(tessari_vault_infrastructure::Error::Refused {
                class: Some(RefusalClass::Retry),
                ..
            })) => Err(Error::Conflict(Conflict::StaleShares)),
            other => other,
        }
    }

    /// Remove the share of `record` with `target`: it stops reaching them at their next request, and the record's
    /// next revision must be sealed under a new record key.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision and for a caller who only reads the record; [`Error::NotFound`] for a
    /// record the caller does not reach or a share it does not have; [`Conflict::StaleRevision`] or
    /// [`Conflict::StaleShares`] when the record moved since it was read.
    pub async fn remove(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        target: &str,
    ) -> Result<()> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ShareRecord,
        )
        .await?;
        let (entity, reached) = self.records.reached(actor, company, record).await?;
        reached.editing()?;
        // Only a target the record is shared with is named in the statement, never the request's text.
        let share = reached
            .shares
            .iter()
            .find(|share| share.target == target)
            .ok_or(Error::NotFound)?;
        let removed = self
            .repository
            .remove(
                &self.place,
                recorded(actor, company, &standing, "record.unshare", record.as_str()),
                ShareRemoval {
                    company,
                    record,
                    revision: entity.revision,
                    share_version: entity.share_version,
                    target: &share.target,
                    vault_key: self.vaults.key(company)?,
                },
            )
            .await;
        match removed {
            Err(Error::Store(tessari_vault_infrastructure::Error::Refused {
                class: Some(RefusalClass::Retry),
                ..
            })) => Err(Error::Conflict(Conflict::StaleShares)),
            other => other,
        }
    }
}
