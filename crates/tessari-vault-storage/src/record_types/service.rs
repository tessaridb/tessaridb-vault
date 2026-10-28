//! Record types a company defines (C5). Admins define them; every member sees which exist and who may use them.

use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{CompanyId, RecordId, RecordTypeId, Roles};

use super::entity::{RecordTypeDefinition, RecordTypeEntity};
use super::repository::{NewType, RecordTypeRepository};
use crate::Result;
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;

/// Record types.
pub struct RecordTypeService {
    repository: Arc<dyn RecordTypeRepository>,
    database: Database,
    place: Place,
}

impl RecordTypeService {
    pub(crate) fn new(
        repository: Arc<dyn RecordTypeRepository>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            database,
            place,
        }
    }

    /// The company's record types.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Refused`] unless the caller is an active member of the open company.
    pub async fn list(
        &self,
        actor: &Actor,
        company: &CompanyId,
    ) -> Result<Vec<RecordTypeDefinition>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadRecord,
        )
        .await?;
        self.repository
            .list(&self.place, company)
            .await?
            .into_iter()
            .map(RecordTypeEntity::into_model)
            .collect()
    }

    /// Define a type whose records `allowed` may create, its definition sealed in the caller's own type-definition
    /// record `definition`.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Refused`] per the decision; [`crate::Error::NotFound`] when `definition` is not the caller's
    /// own type-definition record in the company.
    pub async fn define(
        &self,
        actor: &Actor,
        company: &CompanyId,
        allowed: Roles,
        definition: &RecordId,
    ) -> Result<RecordTypeDefinition> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::DefineRecordType,
        )
        .await?;
        let id = RecordTypeId::from_random(random::bytes()?);
        let now = epoch_seconds();
        self.repository
            .define(
                &self.place,
                recorded(actor, company, &standing, "record_type.define", id.as_str()),
                NewType {
                    company,
                    id: &id,
                    allowed,
                    definition,
                    author: actor.account(),
                    now,
                },
            )
            .await?;
        Ok(RecordTypeDefinition {
            id,
            allowed,
            definition: definition.clone(),
            created: now,
        })
    }

    /// Who may create records of the company's type `id`, or `None` when the company has no such type.
    pub(crate) async fn roles(
        &self,
        company: &CompanyId,
        id: &RecordTypeId,
    ) -> Result<Option<Roles>> {
        self.repository
            .get(&self.place, company, id)
            .await?
            .map(|entity| entity.into_model().map(|definition| definition.allowed))
            .transpose()
    }
}
