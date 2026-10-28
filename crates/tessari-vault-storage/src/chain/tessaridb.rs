//! The chain on TessariDB: a `chains` row per company holding the pinned owner key and the length, and a
//! `statements` row per statement at `<company>_<seq>`.

use async_trait::async_trait;
use tessari_vault_constants::CHAIN_MAX;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value};
use tessari_vault_types::{CompanyId, base64url};

use super::entity::{ChainEntity, StatementEntity};
use super::repository::{Append, ChainRepository};
use crate::access::{GUARD, bind, thrown};
use crate::audit::RECORD;
use crate::{Error, Result};

pub(crate) struct TessaridbChainRepository {
    database: Database,
}

impl TessaridbChainRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "chain",
        reason: format!("{what} did not read back as written"),
    }
}

fn count(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| unreadable("seq"))
}

#[async_trait]
impl ChainRepository for TessaridbChainRepository {
    async fn read(
        &self,
        place: &Place,
        company: &CompanyId,
    ) -> Result<(Option<ChainEntity>, Vec<StatementEntity>)> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM chains:$company; \
                 SELECT * FROM statements WHERE company = $company ORDER BY seq LIMIT $limit \
                 USING INDEX by_company;",
                vec![
                    bind("company", company.as_str()),
                    bind("limit", count(CHAIN_MAX)?),
                ],
            )
            .await?;
        let mut answers = answers.into_iter();
        let chain = answers
            .next()
            .map(Answer::records_into::<ChainEntity>)
            .transpose()
            .map_err(|fault| unreadable(&fault.to_string()))?
            .and_then(|mut rows| rows.pop());
        let statements = answers
            .next()
            .map(Answer::records_into::<StatementEntity>)
            .transpose()
            .map_err(|fault| unreadable(&fault.to_string()))?
            .unwrap_or_default();
        Ok((chain, statements))
    }

    async fn append(
        &self,
        place: &Place,
        mut guard: Vec<(String, Value)>,
        company: &CompanyId,
        append: Append<'_>,
    ) -> Result<()> {
        let Append {
            statement,
            owner_signing,
            author,
            now,
        } = append;
        let seq = count(statement.seq())?;
        guard.extend([
            bind("company", company.as_str()),
            bind("seq", seq),
            bind("previous", seq.saturating_sub(1)),
            bind("statement", format!("{}_{seq}", company.as_str()).as_str()),
            bind("bytes", base64url::encode(statement.bytes()).as_str()),
            bind(
                "signature",
                base64url::encode(statement.signature()).as_str(),
            ),
            bind("author", author.as_str()),
            bind("now", now),
        ]);
        // The chain row is the compare-and-set: the genesis creates it, every later statement moves its length on by
        // one from the length the caller built on.
        let placed = match owner_signing {
            Some(pin) => {
                guard.push(bind("pin", base64url::encode(pin).as_str()));
                "THROW 'stale chain' IF (GET chains:$company).length != NONE; \
                 CREATE chains:$company = { company: $company, owner_signing: $pin, length: 1, created: $now };"
            }
            None => {
                "THROW 'stale chain' IF (GET chains:$company).length != $previous; \
                 UPDATE chains:$company SET length = $seq;"
            }
        };
        let script = format!(
            "BEGIN; {GUARD} {placed} CREATE statements:$statement = {{ company: $company, seq: $seq, \
             bytes: $bytes, signature: $signature, author: $author, created: $now }}; {RECORD} COMMIT;"
        );
        self.database
            .run_at(place, &script, guard)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}
