//! The vouching chain (KH-L3). Owners begin it, owners and admins extend it one statement at a time, and every
//! member reads it whole.

use std::sync::Arc;

use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place, RefusalClass};
use tessari_vault_types::CompanyId;

use super::entity::{Chain, Statement, StatementEntity, pin};
use super::repository::{Append, ChainRepository};
use crate::access::{self, Actor, recorded};
use crate::audit::epoch_seconds;
use crate::{Conflict, Error, Result};

/// The vouching chain.
pub struct ChainService {
    repository: Arc<dyn ChainRepository>,
    database: Database,
    place: Place,
}

impl ChainService {
    pub(crate) fn new(
        repository: Arc<dyn ChainRepository>,
        database: Database,
        place: Place,
    ) -> Self {
        Self {
            repository,
            database,
            place,
        }
    }

    /// The company's whole chain; an empty one when no owner has begun it.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an active member; [`Error::Unreadable`] when the statements kept do
    /// not run from 1 to the chain's length without a gap, because a chain is answered whole or not at all.
    pub async fn read(&self, actor: &Actor, company: &CompanyId) -> Result<Chain> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadKeys,
        )
        .await?;
        let (chain, rows) = self.repository.read(&self.place, company).await?;
        let Some(chain) = chain else {
            return Ok(Chain::default());
        };
        let statements = rows
            .into_iter()
            .map(StatementEntity::into_model)
            .collect::<Result<Vec<_>>>()?;
        let whole = u64::try_from(statements.len()).ok() == u64::try_from(chain.length).ok()
            && statements
                .iter()
                .zip(1_u64..)
                .all(|(statement, seq)| statement.seq() == seq);
        if !whole {
            return Err(Error::Unreadable {
                record: "chain",
                reason: "the statements do not run whole from the genesis".to_owned(),
            });
        }
        Ok(Chain {
            owner_signing: Some(chain.owner_signing()?),
            statements,
        })
    }

    /// Append `statement`; the genesis (`seq` 1) carries `owner_signing`, the owner key the chain is checked from.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] per the decision (an owner begins the chain, an owner or admin extends it);
    /// [`Conflict::StaleChain`] unless `statement` is the next one, including when another statement takes its
    /// place first.
    pub async fn append(
        &self,
        actor: &Actor,
        company: &CompanyId,
        statement: Statement,
        owner_signing: Option<Vec<u8>>,
    ) -> Result<()> {
        let genesis = statement.seq() == 1;
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::Vouch { genesis },
        )
        .await?;
        // Only the genesis pins; a genesis with no pin then fails the compare-and-set as stale.
        let owner_signing = if genesis {
            owner_signing.map(pin).transpose()?
        } else {
            None
        };
        let appended = self
            .repository
            .append(
                &self.place,
                recorded(
                    actor,
                    company,
                    &standing,
                    "chain.append",
                    &statement.seq().to_string(),
                ),
                company,
                Append {
                    statement: &statement,
                    owner_signing: owner_signing.as_deref(),
                    author: actor.account(),
                    now: epoch_seconds(),
                },
            )
            .await;
        match appended {
            // Two statements at one place: the store reports the loser as contention, which is stale here.
            Err(Error::Store(tessari_vault_infrastructure::Error::Refused {
                class: Some(RefusalClass::Retry),
                ..
            })) => Err(Error::Conflict(Conflict::StaleChain)),
            other => other,
        }
    }
}
