//! Record types on TessariDB: one `record_types` row per type, naming the record that holds its sealed definition.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::{CompanyId, RecordTypeId};

use super::entity::RecordTypeEntity;
use super::repository::{NewType, RecordTypeRepository};
use crate::access::{GUARD, bind, thrown};
use crate::audit::RECORD;
use crate::{Result, answers};

pub(crate) struct TessaridbRecordTypeRepository {
    database: Database,
}

impl TessaridbRecordTypeRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

/// The words of a set of roles, as the row stores them.
pub(crate) fn role_words(words: Vec<&'static str>) -> Value {
    Value::Array(words.into_iter().map(Value::from).collect())
}

#[async_trait]
impl RecordTypeRepository for TessaridbRecordTypeRepository {
    async fn list(&self, place: &Place, company: &CompanyId) -> Result<Vec<RecordTypeEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM record_types WHERE company = $company USING INDEX by_company;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        answers::records(answers, "record type")
    }

    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        id: &RecordTypeId,
    ) -> Result<Option<RecordTypeEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                // By address, as records are read.
                "SELECT * FROM (SELECT * FROM record_types:$type LIMIT 2) WHERE company = $company;",
                vec![bind("type", id.as_str()), bind("company", company.as_str())],
            )
            .await?;
        answers::single(answers, "record type")
    }

    async fn define(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        new: NewType<'_>,
    ) -> Result<()> {
        parameters.extend([
            bind("company", new.company.as_str()),
            bind("type", new.id.as_str()),
            ("allowed".to_owned(), role_words(new.allowed.words())),
            bind("definition", new.definition.as_str()),
            bind("author", new.author.as_str()),
            bind("now", new.now),
        ]);
        let script = format!(
            "BEGIN; {GUARD} \
             THROW 'not found' IF (GET records:$definition).company != $company \
             OR (GET records:$definition).author != $author \
             OR (GET records:$definition).record_type != 'record-type'; \
             CREATE record_types:$type = {{ type_id: $type, company: $company, allowed_roles: $allowed, \
             definition: $definition, created: $now }}; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}
