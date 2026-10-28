//! Records (C5, layer 2). Every call decides first; which records a caller reaches is then decided on the record's
//! stored author, inside the query, so another member's record is answered as one that does not exist. Sharing
//! (T3.3) widens that predicate and nothing else.

use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place, RefusalClass};
use tessari_vault_types::{CompanyId, KeyId, RecordId, RecordType, Roles};

use super::entity::{OpenedRecord, Record, RecordEntity, Sealed};
use super::repository::{RecordRepository, Write};
use super::vault::RecordVaults;
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;
use crate::record_types::RecordTypeService;
use crate::{Conflict, Error, Result};

/// Records and their revisions.
pub struct RecordService {
    repository: Arc<dyn RecordRepository>,
    vaults: Arc<RecordVaults>,
    types: Arc<RecordTypeService>,
    database: Database,
    place: Place,
}

impl RecordService {
    pub(crate) fn new(
        repository: Arc<dyn RecordRepository>,
        vaults: Arc<RecordVaults>,
        types: Arc<RecordTypeService>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            vaults,
            types,
            database,
            place,
        }
    }

    /// The records the caller wrote in the company.
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
        self.repository
            .list(&self.place, company, actor.account())
            .await?
            .into_iter()
            .map(RecordEntity::into_model)
            .collect()
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

    /// The record and its latest revision's sealed parts; the read is on the company's trail before it is opened.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision; [`Error::NotFound`] for a record that is not the caller's.
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
        let entity = self.authors(actor, company, record).await?;
        let sealed = self
            .repository
            .open(
                &self.place,
                recorded(actor, company, &standing, "record.read", record.as_str()),
                company,
                &entity,
                self.vaults.key(company)?,
            )
            .await?;
        Ok(OpenedRecord {
            record: entity.into_model()?,
            sealed,
        })
    }

    /// Write a new revision `sealed`, under the record key `key_id`, over the revision `expected` the caller read.
    ///
    /// # Errors
    ///
    /// As [`RecordService::open`], and [`Conflict::StaleRevision`] when the record has moved past `expected`,
    /// including when another revision over it commits first.
    pub async fn revise(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
        expected: i64,
        key_id: &KeyId,
        sealed: &Sealed,
    ) -> Result<Record> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReviseRecord,
        )
        .await?;
        let entity = self.authors(actor, company, record).await?;
        let next = expected
            .checked_add(1)
            .filter(|_| entity.revision == expected)
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
                    key_id,
                    author: actor.account(),
                    sealed,
                    now,
                    vault_key: self.vaults.key(company)?,
                    company_type: None,
                },
                expected,
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
        revised.key_id = key_id.clone();
        revised.updated = now;
        Ok(revised)
    }

    /// The record, when it is the company's and the caller wrote it.
    async fn authors(
        &self,
        actor: &Actor,
        company: &CompanyId,
        record: &RecordId,
    ) -> Result<RecordEntity> {
        self.repository
            .get(&self.place, company, actor.account(), record)
            .await?
            .ok_or(Error::NotFound)
    }
}
