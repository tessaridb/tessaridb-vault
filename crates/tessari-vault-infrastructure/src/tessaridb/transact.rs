//! A transaction the store refused for contention, sent again until one lands.
//!
//! The store reports contention rather than waiting for it (`retry` class), and a transaction it refused wrote
//! nothing, so sending the same `BEGIN ... COMMIT` again is safe. Only a request that is one transaction is sent
//! again: the reads that follow it travel in a request of their own, because a refusal after the commit would send a
//! committed change a second time. Those reads change nothing and are sent again on the same terms.

use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use tessari_vault_constants::{
    STORE_RETRY_ATTEMPTS, STORE_RETRY_FIRST_PAUSE_MS, STORE_RETRY_MAX_PAUSE_MS,
};
use tessaridb_client::{Answer, Request, Value, VaultKey};

use super::connection::bound;
use super::{Database, Place};
use crate::Result;

impl Database {
    /// Run `transaction`, exactly one `BEGIN ... COMMIT;`, at `place`, sending it again while the store answers
    /// `retry`, then the reads in `then` (which may be empty) as a request of their own; one [`Answer`] per
    /// statement of both, in order.
    ///
    /// # Errors
    ///
    /// As [`Database::run_at`]; a `retry` refusal is returned once the attempts are spent.
    pub async fn transact_at(
        &self,
        place: &Place,
        transaction: &str,
        then: &str,
        parameters: Vec<(String, Value)>,
    ) -> Result<Vec<Answer>> {
        let preamble = place.preamble();
        let requests = [transaction, then]
            .into_iter()
            .filter(|script| !script.is_empty())
            .map(|script| bound(format!("{preamble} {script}"), parameters.clone()));
        self.each_until_landed(requests).await
    }

    /// As [`Database::transact_at`], with `key` presented for the keyed vault `vault` at `place` on both requests.
    ///
    /// # Errors
    ///
    /// As [`Database::transact_at`]; the node refuses a key that does not open the vault.
    pub async fn transact_presenting(
        &self,
        place: &Place,
        transaction: &str,
        then: &str,
        parameters: Vec<(String, Value)>,
        vault: &str,
        key: VaultKey,
    ) -> Result<Vec<Answer>> {
        let preamble = place.preamble();
        let requests = [transaction, then]
            .into_iter()
            .filter(|script| !script.is_empty())
            .map(|script| {
                bound(format!("{preamble} {script}"), parameters.clone())
                    .presenting((place.namespace(), place.database(), vault), key.clone())
            });
        self.each_until_landed(requests).await
    }

    /// Run `script`, which only reads, at `place`, sending it again while the store answers `retry`: a read whose
    /// snapshot another commit passed is refused that way, and a refused read changed nothing.
    ///
    /// # Errors
    ///
    /// As [`Database::transact_at`].
    pub async fn read_at(
        &self,
        place: &Place,
        script: &str,
        parameters: Vec<(String, Value)>,
    ) -> Result<Vec<Answer>> {
        let preamble = place.preamble();
        self.until_landed(&bound(format!("{preamble} {script}"), parameters))
            .await
    }

    /// Run `script`, each statement of which may be sent again without changing what it did — a counter set if
    /// absent, then incremented — at `place`, sending it again while the store answers `retry`: the statement it
    /// refused wrote nothing, and the ones before it change nothing a second time.
    ///
    /// # Errors
    ///
    /// As [`Database::transact_at`].
    pub async fn run_again_at(
        &self,
        place: &Place,
        script: &str,
        parameters: Vec<(String, Value)>,
    ) -> Result<Vec<Answer>> {
        let preamble = place.preamble();
        self.until_landed(&bound(format!("{preamble} {script}"), parameters))
            .await
    }

    async fn each_until_landed(
        &self,
        requests: impl Iterator<Item = Request>,
    ) -> Result<Vec<Answer>> {
        let mut answers = Vec::new();
        for request in requests {
            answers.extend(self.until_landed(&request).await?);
        }
        Ok(answers)
    }

    async fn until_landed(&self, request: &Request) -> Result<Vec<Answer>> {
        let random = SystemRandom::new();
        let mut attempt = 0;
        loop {
            match self.exchange(request, 2).await {
                Err(refused) if refused.is_retry() && attempt < STORE_RETRY_ATTEMPTS => {
                    let mut draw = [0_u8; 2];
                    // Without randomness every pause is its ceiling: slower to spread, never wrong.
                    let draw = random
                        .fill(&mut draw)
                        .map_or(u16::MAX, |()| u16::from_le_bytes(draw));
                    tokio::time::sleep(pause(attempt, draw)).await;
                    attempt = attempt.saturating_add(1);
                }
                outcome => return outcome,
            }
        }
    }
}

/// The pause before sending again after `attempt` refusals: a share `draw / u16::MAX` of a ceiling that starts at
/// [`STORE_RETRY_FIRST_PAUSE_MS`] and doubles up to [`STORE_RETRY_MAX_PAUSE_MS`], so writers that lost the same race
/// spread out instead of meeting again.
pub(super) fn pause(attempt: u32, draw: u16) -> Duration {
    let doubling = 1_u64.checked_shl(attempt).unwrap_or(u64::MAX);
    let ceiling_us = STORE_RETRY_FIRST_PAUSE_MS
        .saturating_mul(doubling)
        .min(STORE_RETRY_MAX_PAUSE_MS)
        .saturating_mul(1_000);
    let share_us = ceiling_us
        .saturating_mul(u64::from(draw))
        .checked_div(u64::from(u16::MAX))
        .unwrap_or(ceiling_us);
    Duration::from_micros(share_us)
}

#[cfg(test)]
#[path = "transact_tests.rs"]
mod tests;
