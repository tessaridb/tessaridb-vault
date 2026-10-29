//! How much still rests on each epoch of the service key: an epoch may leave the ring only when nothing names it.

use tessari_vault_infrastructure::custody::TotpSeal;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};

use super::RotationService;
use crate::answers::records;
use crate::enrolments::record::unhex;
use crate::{Error, Result};

/// Per epoch, ascending, how many of each thing it keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remaining {
    /// Company vaults, by the epoch their row records.
    pub companies: Vec<(u32, usize)>,
    /// Sign-in verifiers, by the epoch whose pepper made them; they move when their member signs in.
    pub verifiers: Vec<(u32, usize)>,
    /// TOTP seeds, by the epoch their sealed bytes name.
    pub seeds: Vec<(u32, usize)>,
}

/// One row's epoch column.
struct Epoch(i64);

impl FromRecord for Epoch {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self(row.take("key_epoch")?))
    }
}

/// One credential's sealed seed.
struct Seed(String);

impl FromRecord for Seed {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self(row.take("totp")?))
    }
}

/// Count `epochs`, ascending by epoch.
fn tally(epochs: impl IntoIterator<Item = u32>) -> Vec<(u32, usize)> {
    let mut counted: Vec<(u32, usize)> = Vec::new();
    for epoch in epochs {
        match counted.iter_mut().find(|(held, _)| *held == epoch) {
            Some((_, count)) => *count = count.saturating_add(1),
            None => counted.push((epoch, 1)),
        }
    }
    counted.sort_unstable_by_key(|(epoch, _)| *epoch);
    counted
}

fn epoch(value: i64, record: &'static str) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::Unreadable {
        record,
        reason: "key_epoch did not read back as written".to_owned(),
    })
}

impl RotationService {
    /// What rests on each epoch now.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the store fails; [`Error::Unreadable`] for an epoch or a seed that does not read back.
    pub async fn remaining(&self) -> Result<Remaining> {
        let companies = self
            .database
            .read_at(
                &self.place,
                "SELECT key_epoch FROM groups WHERE kind = 'company';",
                Vec::new(),
            )
            .await?;
        let credentials = self
            .database
            .read_at(
                &self.place,
                "SELECT key_epoch FROM credentials;",
                Vec::new(),
            )
            .await?;
        let seeds = self
            .database
            .read_at(&self.place, "SELECT totp FROM credentials;", Vec::new())
            .await?;
        let companies = records::<Epoch>(companies, "company")?
            .into_iter()
            .map(|Epoch(value)| epoch(value, "company"))
            .collect::<Result<Vec<_>>>()?;
        let verifiers = records::<Epoch>(credentials, "credential")?
            .into_iter()
            .map(|Epoch(value)| epoch(value, "credential"))
            .collect::<Result<Vec<_>>>()?;
        let seeds = records::<Seed>(seeds, "credential")?
            .into_iter()
            .map(|Seed(text)| {
                unhex(&text)
                    .and_then(|sealed| TotpSeal::epoch_of(&sealed))
                    .ok_or(Error::Unreadable {
                        record: "credential",
                        reason: "a sealed seed that names no epoch".to_owned(),
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Remaining {
            companies: tally(companies),
            verifiers: tally(verifiers),
            seeds: tally(seeds),
        })
    }
}
