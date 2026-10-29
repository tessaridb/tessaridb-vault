//! Deleting a company: an owner asks with a statement their own registered key signed, so the server can never ask
//! on an owner's behalf; the platform admin confirms; the company then waits, read-only and cancellable by an owner,
//! before the operator's command drops its vault.

use tessari_vault_core::access::Action;
use tessari_vault_core::company_drop::{Act, statement, verify};
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value};
use tessari_vault_types::{CompanyId, base64url};

use crate::access::{self, Actor, GUARD, bind, membership_id, recorded, thrown};
use crate::audit::{RECORD, epoch_seconds};
use crate::{Conflict, Error, Result};

/// How far a signed statement's time may sit from now.
const STATEMENT_WINDOW_SECONDS: u64 = 600;

/// Where a deletion stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropState {
    /// An owner asked; the platform admin has not confirmed.
    Requested,
}

impl DropState {
    /// The word the API answers.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Requested => "requested",
        }
    }
}

/// Company deletion.
pub struct CompanyDropService {
    database: Database,
    place: Place,
}

impl CompanyDropService {
    pub(crate) const fn new(database: Database, place: Place) -> Self {
        Self { database, place }
    }

    /// An owner asks for `company`'s deletion with `signature` over the request statement for `at`, made by their own
    /// registered key in the company; recorded on the trail.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone but an active owner; [`Conflict::DropUnsigned`] when the signature is not the
    /// caller's registered key's over that statement, or `at` is not within ten minutes of now.
    pub async fn request(
        &self,
        actor: &Actor,
        company: &CompanyId,
        at: u64,
        signature: &[u8],
    ) -> Result<DropState> {
        let standing = access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::DropCompany,
        )
        .await?;
        self.signed(actor, company, Act::Request, at, signature)
            .await?;
        let mut parameters = recorded(
            actor,
            company,
            &standing,
            "company.drop.request",
            company.as_str(),
        );
        parameters.extend([
            bind("company", company.as_str()),
            bind("by", actor.account().as_str()),
            bind("at", epoch_seconds()),
            bind("signature", base64url::encode(signature).as_str()),
        ]);
        let script = format!(
            "BEGIN; {GUARD} \
             UPSERT company_drops:$company = {{ company: $company, state: 'requested', requested_by: $by, \
             requested_at: $at, signature: $signature }}; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(DropState::Requested)
    }

    /// Refuse unless `signature` is the caller's registered signing key's over `act` on `company` at `at`, and `at`
    /// is close to now.
    async fn signed(
        &self,
        actor: &Actor,
        company: &CompanyId,
        act: Act,
        at: u64,
        signature: &[u8],
    ) -> Result<()> {
        let now = u64::try_from(epoch_seconds()).unwrap_or(0);
        if now.abs_diff(at) > STATEMENT_WINDOW_SECONDS {
            return Err(Error::Conflict(Conflict::DropUnsigned));
        }
        let answers = self
            .database
            .read_at(
                &self.place,
                "RETURN (GET member_keys:$keys).signing_public;",
                vec![bind(
                    "keys",
                    membership_id(company, actor.account()).as_str(),
                )],
            )
            .await?;
        let Some(Answer::Value {
            value: Value::String(encoded),
            ..
        }) = answers.into_iter().next()
        else {
            return Err(Error::Conflict(Conflict::DropUnsigned));
        };
        let public = base64url::decode(&encoded).map_err(|_| Error::Unreadable {
            record: "key set",
            reason: "signing_public did not read back as written".to_owned(),
        })?;
        verify(&public, &statement(act, company.as_str(), at), signature)
            .map_err(|_| Error::Conflict(Conflict::DropUnsigned))
    }
}
