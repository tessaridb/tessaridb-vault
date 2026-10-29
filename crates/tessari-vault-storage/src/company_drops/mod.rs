//! Deleting a company: an owner asks with a statement their own registered key signed, so the server can never ask
//! on an owner's behalf; the platform admin confirms; the company then waits, read-only and cancellable by an owner,
//! before the operator's command drops its vault.

use tessari_vault_constants::COMPANY_DROP_WAIT_SECONDS;
mod execute;
mod reads;

use tessari_vault_core::access::Action;
use tessari_vault_core::company_drop::{Act, statement, verify};
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value};
use tessari_vault_types::{CompanyId, base64url};

use crate::access::{self, Actor, GUARD, bind, membership_id, recorded, thrown};
use crate::audit::{self, Event, RECORD, epoch_seconds};
use crate::{Conflict, Error, Result};

/// How far a signed statement's time may sit from now.
const STATEMENT_WINDOW_SECONDS: u64 = 600;

/// Where a deletion stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropState {
    /// An owner asked; the platform admin has not confirmed.
    Requested,
    /// Confirmed: the company is read-only and is deleted once `drop_after` passes, unless an owner cancels.
    Scheduled {
        /// When it may be carried out, in seconds since 1970.
        drop_after: i64,
    },
    /// An owner cancelled; the company is as it was.
    Cancelled,
}

impl DropState {
    /// The word the API answers.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Scheduled { .. } => "scheduled",
            Self::Cancelled => "cancelled",
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
            "BEGIN; {GUARD} THROW 'drop scheduled' IF (GET company_drops:$company).state = 'scheduled'; \
             UPSERT company_drops:$company = {{ company: $company, state: 'requested', requested_by: $by, \
             requested_at: $at, signature: $signature }}; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(DropState::Requested)
    }

    /// The platform admin confirms the deletion an owner asked for: the company becomes read-only and waits seven
    /// days, on its trail.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone but the platform admin; [`Error::NotFound`] when no owner asked, or the company
    /// is not open.
    pub async fn confirm(&self, actor: &Actor, company: &CompanyId) -> Result<DropState> {
        actor.platform()?;
        let drop_after = epoch_seconds().saturating_add(COMPANY_DROP_WAIT_SECONDS);
        let mut parameters = audit::binds(
            actor,
            &Event {
                company: company.as_str(),
                action: "company.drop.confirm",
                target: company.as_str(),
            },
        );
        parameters.extend([
            bind("company", company.as_str()),
            bind("by", actor.account().as_str()),
            bind("after", drop_after),
        ]);
        let script = format!(
            "BEGIN; THROW 'not found' IF (GET company_drops:$company).state != 'requested' \
             OR (GET groups:$company).state != 'active'; \
             UPDATE company_drops:$company SET state = 'scheduled', confirmed_by = $by, drop_after = $after; \
             UPDATE groups:$company SET state = 'dropping'; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(DropState::Scheduled { drop_after })
    }

    /// An owner cancels a deletion still waiting, with a cancel statement their registered key signed; the company
    /// is open again, on its trail.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for anyone but an active owner; [`Conflict::DropUnsigned`] for a statement not signed now
    /// by their key; [`Error::NotFound`] when no deletion waits.
    pub async fn cancel(
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
        self.signed(actor, company, Act::Cancel, at, signature)
            .await?;
        let mut parameters = recorded(
            actor,
            company,
            &standing,
            "company.drop.cancel",
            company.as_str(),
        );
        parameters.push(bind("company", company.as_str()));
        let script = format!(
            "BEGIN; {GUARD} THROW 'not found' IF (GET company_drops:$company) = NONE; \
             DELETE company_drops:$company; \
             UPDATE groups:$company SET state = 'active' WHERE state = 'dropping'; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(&self.place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(DropState::Cancelled)
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
