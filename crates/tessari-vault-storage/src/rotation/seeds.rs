//! Resealing every TOTP seed under the current epoch: each sealed seed names the epoch that sealed it, so it is
//! opened under that epoch, sealed again under the current one, and written back only if it is still the value read.

use tessari_vault_infrastructure::custody::TotpSeal;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};

use super::RotationService;
use crate::Result;
use crate::access::bind;
use crate::answers::records;
use crate::enrolments::record::unhex;
use crate::sessions::hex;

/// One credential's sealed seed as the sweep reads it.
struct Sealed {
    account: String,
    totp: String,
    factor: String,
}

impl FromRecord for Sealed {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            account: row.take("account_id")?,
            totp: row.take("totp")?,
            factor: row.take("totp_factor")?,
        })
    }
}

/// What the sweep did.
pub(super) struct Swept {
    pub(super) resealed: usize,
    pub(super) unopened: usize,
}

impl RotationService {
    /// Seal again, under the current epoch, every seed sealed under another.
    pub(super) async fn reseal_seeds(&self) -> Result<Swept> {
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT account_id, totp, totp_factor FROM credentials;",
                Vec::new(),
            )
            .await?;
        let current = self.seal.current_epoch();
        let mut swept = Swept {
            resealed: 0,
            unopened: 0,
        };
        for held in records::<Sealed>(answers, "credential")? {
            let Some(sealed) = unhex(&held.totp) else {
                swept.unopened = swept.unopened.saturating_add(1);
                continue;
            };
            if TotpSeal::epoch_of(&sealed) == Some(current) {
                continue;
            }
            let Ok(seed) = self.seal.open(&held.account, &held.factor, &sealed) else {
                swept.unopened = swept.unopened.saturating_add(1);
                continue;
            };
            let resealed = hex(&self.seal.seal(&held.account, &held.factor, &seed)?);
            self.database
                .run_at(
                    &self.place,
                    "UPDATE credentials:$a SET totp = $new WHERE totp = $old;",
                    vec![
                        bind("a", held.account.as_str()),
                        bind("new", resealed.as_str()),
                        bind("old", held.totp.as_str()),
                    ],
                )
                .await?;
            swept.resealed = swept.resealed.saturating_add(1);
        }
        Ok(swept)
    }
}
