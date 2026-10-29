//! A person's own passkeys: listing them, and removing one. Adding and removing a passkey changes how the person signs
//! in everywhere, so each goes on the trail of every company they belong to, in the transaction that makes it.

use tessari_vault_constants::KEY_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row, Value};

use super::PasskeyService;
use crate::access::{Actor, bind, thrown};
use crate::audit::{self, Event};
use crate::{Result, answers};

/// One of the caller's passkeys, as their account page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passkey {
    /// The credential id, base64url.
    pub credential: String,
    /// When it was registered, in seconds since the epoch.
    pub created: i64,
    /// Whether the authenticator may copy it to the person's other devices.
    pub backup_eligible: bool,
}

impl FromRecord for Passkey {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            credential: row.take("credential")?,
            created: row.take("created")?,
            backup_eligible: row.take("backup_eligible")?,
        })
    }
}

/// A `memberships` row as the trail reads it: which company.
struct Company {
    company: String,
}

impl FromRecord for Company {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            company: row.take("company")?,
        })
    }
}

impl PasskeyService {
    /// The statements that put `action` on the trail of every company `actor` belongs to, and their values.
    pub(super) async fn trail(
        &self,
        actor: &Actor,
        action: &'static str,
    ) -> Result<(String, Vec<(String, Value)>)> {
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT company FROM memberships WHERE account = $account LIMIT $limit;",
                vec![
                    bind("account", actor.account().as_str()),
                    bind("limit", KEY_LIST_MAX),
                ],
            )
            .await?;
        let companies: Vec<Company> = answers::records(answers, "membership")?;
        let mut records = String::new();
        let mut parameters = Vec::new();
        for (n, row) in companies.iter().enumerate() {
            records.push_str(&audit::record_numbered(n));
            records.push(' ');
            parameters.extend(audit::binds_numbered(
                n,
                actor,
                &Event {
                    company: &row.company,
                    action,
                    target: actor.account().as_str(),
                },
            ));
        }
        Ok((records, parameters))
    }

    /// The caller's own passkeys.
    ///
    /// # Errors
    ///
    /// When the store fails or a row does not read back.
    pub async fn list(&self, actor: &Actor) -> Result<Vec<Passkey>> {
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT credential, backup_eligible, time::unix(created) AS created FROM passkeys \
                 WHERE account = $account LIMIT $limit USING INDEX by_account;",
                vec![
                    bind("account", actor.account().as_str()),
                    bind("limit", KEY_LIST_MAX),
                ],
            )
            .await?;
        answers::records(answers, "passkey")
    }

    /// Remove the caller's own passkey `credential`, on the trail of each of their companies.
    ///
    /// # Errors
    ///
    /// [`crate::Error::NotFound`] for a credential that is not the caller's, or not registered.
    pub async fn remove(&self, actor: &Actor, credential: &str) -> Result<()> {
        let (records, mut parameters) = self.trail(actor, "passkey.remove").await?;
        parameters.extend([
            bind("account", actor.account().as_str()),
            bind("credential", credential),
        ]);
        let script = format!(
            "BEGIN; LET $held = SELECT count(*) AS n FROM passkeys WHERE credential = $credential \
             AND account = $account; THROW 'not found' IF $held[0].n != 1; \
             DELETE FROM passkeys WHERE credential = $credential AND account = $account; {records}COMMIT;"
        );
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}
