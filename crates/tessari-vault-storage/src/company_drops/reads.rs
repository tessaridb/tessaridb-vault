//! Reading where deletions stand: one company's for its owner, and every open one for the platform admin.

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::CompanyId;

use super::{CompanyDropService, DropState};
use crate::access::{self, Actor, bind};
use crate::answers::records;
use crate::{Error, Result};

/// An open deletion's row.
struct Open {
    company: String,
    state: String,
    drop_after: Option<i64>,
}

impl FromRecord for Open {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            company: row.take("company")?,
            state: row.take("state")?,
            drop_after: row.take("drop_after")?,
        })
    }
}

impl Open {
    fn state(&self) -> DropState {
        match self.drop_after {
            Some(drop_after) if self.state == "scheduled" => DropState::Scheduled { drop_after },
            _ => DropState::Requested,
        }
    }
}

/// The most open deletions one listing answers; each is a company asking to leave, so the queue stays short.
const OPEN_LISTED: i64 = 1000;

impl CompanyDropService {
    /// Where `company`'s open deletion stands, for an owner of it or the platform admin; `None` when none is open.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone else.
    pub async fn standing(&self, actor: &Actor, company: &CompanyId) -> Result<Option<DropState>> {
        if !actor.is_platform_admin() {
            access::authorize(
                &self.database,
                &self.place,
                actor,
                company,
                Action::DropCompany,
            )
            .await?;
        }
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT company, state, drop_after FROM company_drops WHERE company = $company;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        Ok(records::<Open>(answers, "company drop")?
            .first()
            .map(Open::state))
    }

    /// Every open deletion, oldest request first, for the platform admin to confirm or watch.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone but the platform admin.
    pub async fn open(&self, actor: &Actor) -> Result<Vec<(CompanyId, DropState)>> {
        actor.platform()?;
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT company, state, drop_after FROM company_drops ORDER BY requested_at LIMIT $most;",
                vec![bind("most", OPEN_LISTED)],
            )
            .await?;
        records::<Open>(answers, "company drop")?
            .iter()
            .map(|open| {
                CompanyId::parse(&open.company)
                    .map(|company| (company, open.state()))
                    .map_err(|_| Error::Unreadable {
                        record: "company drop",
                        reason: "company did not read back as written".to_owned(),
                    })
            })
            .collect()
    }
}
