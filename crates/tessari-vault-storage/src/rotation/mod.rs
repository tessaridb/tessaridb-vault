//! Rotating the service key: moving every company vault to the ring's current epoch.
//!
//! A vault's key is re-wrapped by an act on the wire, which cannot share a transaction with the company row, and the
//! store throttles the whole node after three wrong keys in a row; so each company is first marked `key_moving`, which
//! turns its requests into a retryable conflict that presents no key, then re-wrapped, then given its new epoch and
//! unmarked. A run cut short between the two is finished by the next one, which finds the vault already opening under
//! the current epoch's key.

use std::sync::Arc;

use tessari_vault_infrastructure::Error as StoreError;
use tessari_vault_infrastructure::custody::VaultKeys;
use tessari_vault_infrastructure::tessaridb::{
    Database, FromRecord, MappingFault, Place, RefusalClass, Row,
};
use tessari_vault_types::CompanyId;

use crate::access::bind;
use crate::answers::records;
use crate::records::RecordVaults;
use crate::{Error, Result};

/// What one run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotated {
    /// Companies whose vault was re-wrapped to the current epoch.
    pub moved: usize,
    /// Companies whose vault a run cut short had re-wrapped already, and whose row this run finished.
    pub repaired: usize,
    /// Companies whose vault opened under neither epoch: left marked, so their requests are refused without a key.
    pub stranded: Vec<CompanyId>,
}

/// One company row as the rotation reads it.
struct Keyed {
    id: String,
    epoch: i64,
}

impl FromRecord for Keyed {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("group_id")?,
            epoch: row.take("key_epoch")?,
        })
    }
}

/// Which key a vault opened under.
enum Opens {
    Recorded,
    Current,
    Neither,
}

/// The rotation, run by the operator.
pub struct RotationService {
    database: Database,
    place: Place,
    keys: Arc<VaultKeys>,
}

impl RotationService {
    pub(crate) const fn new(database: Database, place: Place, keys: Arc<VaultKeys>) -> Self {
        Self {
            database,
            place,
            keys,
        }
    }

    /// Move every company vault not yet on the current epoch, or marked moving, to the current epoch.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the store fails, a throttled key included: the run stops rather than spend more wrong
    /// keys, and the next run continues where it stopped.
    pub async fn rotate(&self) -> Result<Rotated> {
        let current = self.keys.current_epoch();
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT group_id, key_epoch FROM groups WHERE kind = 'company' \
                 AND (key_epoch != $current OR key_moving = true);",
                vec![bind("current", i64::from(current))],
            )
            .await?;
        let mut rotated = Rotated {
            moved: 0,
            repaired: 0,
            stranded: Vec::new(),
        };
        for keyed in records::<Keyed>(answers, "company")? {
            let company = CompanyId::parse(&keyed.id).map_err(|_| unreadable("group_id"))?;
            let recorded = u32::try_from(keyed.epoch).map_err(|_| unreadable("key_epoch"))?;
            self.mark(&company, recorded).await?;
            match self.opens(&company, recorded, current).await? {
                Opens::Recorded => {
                    if recorded != current {
                        self.database
                            .change_keyed_vault(
                                &self.place,
                                &RecordVaults::name(&company),
                                &self.keys.company(recorded, company.as_str())?,
                                &self.keys.company(current, company.as_str())?,
                            )
                            .await?;
                    }
                    rotated.moved = rotated.moved.saturating_add(1);
                }
                Opens::Current => rotated.repaired = rotated.repaired.saturating_add(1),
                Opens::Neither => {
                    rotated.stranded.push(company);
                    continue;
                }
            }
            self.settle(&company, recorded, current).await?;
        }
        Ok(rotated)
    }

    /// Mark `company` moving, while its row still names `epoch`.
    async fn mark(&self, company: &CompanyId, epoch: u32) -> Result<()> {
        self.database
            .run_at(
                &self.place,
                "UPDATE groups:$c SET key_moving = true WHERE key_epoch = $epoch;",
                vec![bind("c", company.as_str()), bind("epoch", i64::from(epoch))],
            )
            .await?;
        Ok(())
    }

    /// Give `company` the epoch `current` and unmark it, while its row still names `recorded`.
    async fn settle(&self, company: &CompanyId, recorded: u32, current: u32) -> Result<()> {
        self.database
            .run_at(
                &self.place,
                "UPDATE groups:$c SET key_epoch = $current, key_moving = false WHERE key_epoch = $recorded;",
                vec![
                    bind("c", company.as_str()),
                    bind("current", i64::from(current)),
                    bind("recorded", i64::from(recorded)),
                ],
            )
            .await?;
        Ok(())
    }

    /// Which key opens `company`'s vault: the one its row records, else the current one. The probe reads an id the
    /// server never writes, so it touches no record; at most one wrong key is presented per company.
    async fn opens(&self, company: &CompanyId, recorded: u32, current: u32) -> Result<Opens> {
        if self.probe(company, recorded).await? {
            return Ok(Opens::Recorded);
        }
        if recorded != current && self.probe(company, current).await? {
            return Ok(Opens::Current);
        }
        Ok(Opens::Neither)
    }

    async fn probe(&self, company: &CompanyId, epoch: u32) -> Result<bool> {
        let Ok(key) = self.keys.company(epoch, company.as_str()) else {
            return Ok(false);
        };
        let vault = RecordVaults::name(company);
        // The name was built from a checked company id; it is grammar and cannot be bound.
        match self
            .database
            .run_presenting(
                &self.place,
                &format!("REVEAL body FROM {vault}:'key-probe';"),
                Vec::new(),
                &vault,
                key,
            )
            .await
        {
            Ok(_) => Ok(true),
            Err(StoreError::Refused {
                class: Some(RefusalClass::Invalid),
                ..
            }) => Ok(false),
            Err(failure) => Err(failure.into()),
        }
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "company",
        reason: format!("{what} did not read back as written"),
    }
}
