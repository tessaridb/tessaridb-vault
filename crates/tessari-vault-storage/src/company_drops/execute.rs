//! Carrying out a deletion whose wait has passed: the company's vault is dropped, which destroys the key its record
//! bodies and small files were sealed under, and its rows, graph edges and bucket parts go with it in the same
//! transaction. Its audit trail stays, ending with the drop, because the trail is what outlives the company.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row, Value};
use tessari_vault_types::{AccountId, CompanyId};

use super::CompanyDropService;
use crate::access::{Actor, bind, thrown};
use crate::answers::records;
use crate::audit::{self, Event, RECORD, epoch_seconds};
use crate::files::bucket_path;
use crate::records::{RecordVaults, parts};
use crate::{Error, Result};

/// The tables that hold a company's rows by their `company` field.
const COMPANY_TABLES: [&str; 14] = [
    "memberships",
    "records",
    "record_types",
    "files",
    "member_keys",
    "chains",
    "statements",
    "group_keys",
    "shares",
    "recovery_keys",
    "escrows",
    "recovery_notices",
    "recovery_requests",
    "groups",
];

/// A deletion whose wait has passed.
struct Due {
    company: String,
    confirmed_by: String,
}

impl FromRecord for Due {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            company: row.take("company")?,
            confirmed_by: row.take("confirmed_by")?,
        })
    }
}

/// One id column.
struct Id(String);

impl FromRecord for Id {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self(row.take("id")?))
    }
}

/// A department or team of the company and the group it sits in.
struct Placed {
    id: String,
    parent: String,
}

impl FromRecord for Placed {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("group_id")?,
            parent: row.take("parent")?,
        })
    }
}

/// A file of the company, for its bucket parts.
struct Kept {
    id: String,
    size: i64,
    kept: String,
}

impl FromRecord for Kept {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("file_id")?,
            size: row.take("size")?,
            kept: row.take("kept")?,
        })
    }
}

impl CompanyDropService {
    /// Carry out every confirmed deletion whose wait has passed; the companies deleted.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the store fails; a company the run could not delete stays scheduled for the next run.
    pub async fn drop_due(&self) -> Result<Vec<CompanyId>> {
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT company, confirmed_by FROM company_drops WHERE state = 'scheduled' AND drop_after <= $now;",
                vec![bind("now", epoch_seconds())],
            )
            .await?;
        let mut dropped = Vec::new();
        for due in records::<Due>(answers, "company drop")? {
            let company = CompanyId::parse(&due.company).map_err(|_| unreadable("company"))?;
            let by = AccountId::parse(&due.confirmed_by).map_err(|_| unreadable("confirmed_by"))?;
            self.drop_one(&company, &Actor::new(by, true)).await?;
            dropped.push(company);
        }
        Ok(dropped)
    }

    async fn drop_one(&self, company: &CompanyId, by: &Actor) -> Result<()> {
        let c = company.as_str();
        let mut parameters = audit::binds(
            &by.clone().with_reason(
                "the waiting period after an owner's request and its confirmation passed"
                    .to_owned(),
            ),
            &Event {
                company: c,
                action: "company.drop",
                target: c,
            },
        );
        parameters.push(bind("company", c));
        let mut script = String::from(
            "BEGIN; THROW 'standing changed' IF (GET groups:$company).state != 'dropping' \
             OR (GET company_drops:$company).state != 'scheduled'; ",
        );
        let groups = records::<Placed>(
            self.database
                .read_at(
                    &self.place,
                    "SELECT group_id, parent FROM groups WHERE company = $company AND kind != 'company';",
                    vec![bind("company", c)],
                )
                .await?,
            "group",
        )?;
        for (g, group) in groups.iter().enumerate() {
            let members = records::<Id>(
                self.database
                    .read_at(
                        &self.place,
                        "SELECT account_id AS id FROM groups:$group<-in_group<-accounts;",
                        vec![bind("group", group.id.as_str())],
                    )
                    .await?,
                "group member",
            )?;
            for (m, member) in members.iter().enumerate() {
                script.push_str(&format!(
                    "DELETE accounts:$m{g}_{m}->in_group->groups:$g{g}; "
                ));
                parameters.push(bind(&format!("m{g}_{m}"), member.0.as_str()));
            }
            script.push_str(&format!("DELETE groups:$g{g}->part_of->groups:$p{g}; "));
            parameters.push(bind(&format!("g{g}"), group.id.as_str()));
            parameters.push(bind(&format!("p{g}"), group.parent.as_str()));
        }
        let files = records::<Kept>(
            self.database
                .read_at(
                    &self.place,
                    "SELECT file_id, size, kept FROM files WHERE company = $company;",
                    vec![bind("company", c)],
                )
                .await?,
            "file",
        )?;
        for (f, file) in files
            .iter()
            .enumerate()
            .filter(|(_, file)| file.kept == "bucket")
        {
            for part in 0..parts(file.size)? {
                script.push_str(&format!("DELETE record_files:$f{f}_{part}; "));
                parameters.push(bind(
                    &format!("f{f}_{part}"),
                    bucket_path(c, &file.id, part).as_str(),
                ));
            }
        }
        let held: Vec<Value> = records::<Id>(
            self.database
                .read_at(
                    &self.place,
                    "SELECT record_id AS id FROM records WHERE company = $company;",
                    vec![bind("company", c)],
                )
                .await?,
            "record",
        )?
        .into_iter()
        .map(|Id(id)| Value::from(id.as_str()))
        .collect();
        parameters.push(("records".to_owned(), Value::Array(held)));
        script.push_str("DELETE FROM revisions WHERE record IN $records; ");
        for table in COMPANY_TABLES {
            script.push_str(&format!("DELETE FROM {table} WHERE company = $company; "));
        }
        // The vault's name is built from a checked company id; it is grammar and cannot be bound.
        let vault = RecordVaults::name(company);
        script.push_str(&format!(
            "DELETE company_drops:$company; DROP VAULT {vault}; {RECORD} COMMIT;"
        ));
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "company drop",
        reason: format!("{what} did not read back as written"),
    }
}
