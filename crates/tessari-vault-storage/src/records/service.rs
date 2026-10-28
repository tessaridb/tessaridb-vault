//! Records (C5, layer 2). Every call decides first; which records a caller reaches is then decided on the record's
//! stored author and its shares, read from the store, so a record nobody shared with the caller is answered as one
//! that does not exist.

use std::collections::HashSet;
use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place, RefusalClass};
use tessari_vault_types::{CompanyId, KeyId, RecordId, RecordType, Roles};

use super::entity::{OpenedRecord, Record, RecordEntity, Revision, Sealed};
use super::repository::{RecordRepository, Write};
use super::vault::RecordVaults;
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;
use crate::record_types::RecordTypeService;
use crate::shares::{Reach, Reached};
use crate::{Conflict, Error, Result};

/// Records and their revisions.
pub struct RecordService {
    repository: Arc<dyn RecordRepository>,
    vaults: Arc<RecordVaults>,
    types: Arc<RecordTypeService>,
    reach: Arc<Reach>,
    database: Database,
    place: Place,
}

impl RecordService {
    pub(crate) fn new(
        repository: Arc<dyn RecordRepository>,
        vaults: Arc<RecordVaults>,
        types: Arc<RecordTypeService>,
        reach: Arc<Reach>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            vaults,
            types,
            reach,
            database,
            place,
        }
    }

    /// The records the caller wrote in the company, and those shared with them.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member of the open company.
    pub async fn list(&self, actor: &Actor, company: &CompanyId) -> Result<Vec<Record>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadRecord,
        )
        .await?;
        let mut records = self
            .repository
            .list(&self.place, company, actor.account())
            .await?;
        let own: HashSet<String> = records.iter().map(|record| record.id.clone()).collect();
        let targets = self.reach.targets(company, actor.account()).await?;
        let shared: Vec<String> = self
            .reach
            .shared(company, &targets)
            .await?
            .into_iter()
            .filter(|id| !own.contains(id))
            .collect();
        records.extend(self.repository.among(&self.place, company, shared).await?);
        records.into_iter().map(RecordEntity::into_model).collect()
    }

    /// Keep a new record whose first revision is `sealed`, under the record key `key_id`.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::Store`] when the store fails.
    pub async fn create(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record_type: &RecordType,
        key_id: &KeyId,
        sealed: &Sealed,
    ) -> Result<Record> {
        let (allowed, company_type) = match record_type {
            // Built-in types always carry their roles; the narrower set is the fallback that refuses.
            RecordType::BuiltIn(_) => (
                record_type.built_in_roles().unwrap_or(Roles::MANAGERS),
                None,
            ),
            RecordType::Company(id) => {
                // Visibility first, so a caller outside the company learns nothing about its types.
                access::authorize(
                    &self.database,
                    &self.place,
                    actor,
                    company,
                    Action::ReadRecord,
                )
                .await?;
                let roles = self
                    .types
                    .roles(company, id)
                    .await?
                    .ok_or(Error::NotFound)?;
                (roles, Some((id, roles)))
            }
        };
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::CreateRecord { allowed },
        )
        .await?;
        let type_text = record_type.to_string();
        let id = RecordId::from_random(random::bytes()?);
        let now = epoch_seconds();
        self.repository
            .create(
                &self.place,
                recorded(actor, company, &standing, "record.create", id.as_str()),
                Write {
                    company,
                    record: &id,
                    revision: 1,
                    record_type: &type_text,
                    key_id,
                    author: actor.account(),
                    sealed,
                    now,
                    vault_key: self.vaults.key(company)?,
                    company_type,
                    shares: 0,
                    extra: &[],
                },
            )
            .await?;
        Ok(Record {
            id,
            record_type: record_type.clone(),
            revision: 1,
            key_id: key_id.clone(),
            author: actor.account().clone(),
            created: now,
            updated: now,
        })
    }

    /// The record and its latest revision's sealed parts, with the wrap of its key the caller can open; the read is
    /// on the company's trail before it is opened.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a record the caller does not reach.
    pub async fn open(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
    ) -> Result<OpenedRecord> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadRecord,
        )
        .await?;
        let (entity, reached) = self.reached(actor, company, record).await?;
        let own_key = self.reach.member_key(company, actor.account()).await?;
        let opened = self
            .repository
            .open(
                &self.place,
                recorded(actor, company, &standing, "record.read", record.as_str()),
                company,
                &entity,
                self.vaults.key(company)?,
            )
            .await?;
        let (recipient, sealed) = opened.for_caller(&reached, own_key.as_deref())?;
        Ok(OpenedRecord {
            record: entity.into_model()?,
            sealed,
            recipient,
        })
    }

    /// Write a new revision `sealed`, under the record key `key_id`, over the revision `expected` the caller read;
    /// `wraps` carries the record key to every recipient the record keeps but the caller's own.
    ///
    /// # Errors
    ///
    /// As [`RecordService::open`], [`Error::Refused`] for a caller who only reads it, [`Conflict::StaleRevision`]
    /// when the record has moved past `expected`, including when another revision over it commits first, and
    /// [`Conflict::StaleShares`] unless `wraps` names exactly the author's key and the shares' recipients.
    pub async fn revise(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        revision: Revision<'_>,
    ) -> Result<Record> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReviseRecord,
        )
        .await?;
        let (entity, reached) = self.reached(actor, company, record).await?;
        reached.editing()?;
        self.reach
            .revisable(
                company,
                actor.account(),
                &entity.author,
                &reached,
                revision.sealed.author_key(),
                revision.wraps,
            )
            .await?;
        let next = revision
            .expected
            .checked_add(1)
            .filter(|_| entity.revision == revision.expected)
            .ok_or(Error::Conflict(Conflict::StaleRevision))?;
        let now = epoch_seconds();
        let written = self
            .repository
            .revise(
                &self.place,
                recorded(actor, company, &standing, "record.revise", record.as_str()),
                Write {
                    company,
                    record,
                    revision: next,
                    record_type: &entity.record_type,
                    key_id: revision.key_id,
                    author: actor.account(),
                    sealed: revision.sealed,
                    now,
                    vault_key: self.vaults.key(company)?,
                    company_type: None,
                    shares: entity.shares,
                    extra: revision.wraps,
                },
                revision.expected,
            )
            .await;
        match written {
            // Two revisions over one: the store reports the loser as contention, which is the stale case here.
            Err(Error::Store(tessari_vault_infrastructure::Error::Refused {
                class: Some(RefusalClass::Retry),
                ..
            })) => return Err(Error::Conflict(Conflict::StaleRevision)),
            other => other?,
        }
        let mut revised = entity.into_model()?;
        revised.revision = next;
        revised.key_id = revision.key_id.clone();
        revised.updated = now;
        Ok(revised)
    }

    /// The record and what the caller holds on it, when it is the company's and some path reaches the caller.
    pub(crate) async fn reached(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
    ) -> Result<(RecordEntity, Reached)> {
        let entity = self
            .repository
            .get(&self.place, company, record)
            .await?
            .ok_or(Error::NotFound)?;
        let reached = self
            .reach
            .of(company, actor.account(), &entity.id, &entity.author)
            .await?
            .ok_or(Error::NotFound)?;
        Ok((entity, reached))
    }
}
