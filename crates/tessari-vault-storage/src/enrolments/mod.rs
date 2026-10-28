//! Enrolment: the single-use link an account without a credential sets its own password and TOTP through.
//!
//! The link carries 256 random bits; the store holds only their SHA-256 digest, under which the account (and, once
//! enrolment has begun, the sealed pending seed) is kept with an expiry. A newer link for the same account replaces
//! the older one. The record is rewritten with `EXPIRE TTL`, never `UPDATE`, because an update drops a space key's
//! expiry (dogfood DB-V12). Completing it creates the credential and consumes the link in one transaction, so a link
//! works once and an account is enrolled once.

pub(crate) mod record;

use std::sync::Arc;

use tessari_vault_constants::{ENROLMENT_LIFETIME_HOURS, ENROLMENT_TOKEN_BYTES, TOTP_SECRET_BYTES};
use tessari_vault_core::totp;
use tessari_vault_infrastructure::custody::{Passwords, TotpSeal};
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::{AccountId, Password, PasswordError};

use crate::access::{bind, thrown};
use crate::accounts::AccountService;
use crate::sessions::{SessionToken, digest_key, hex};
use crate::{Error, Result};
use record::{field, now_secs, unhex};

/// The issuer an authenticator app shows beside the account.
const ISSUER: &str = "TessariDB Vault";

/// An enrolment link's token as it is handed over; printed only through [`EnrolmentToken::expose`].
pub struct EnrolmentToken(String);

impl std::fmt::Debug for EnrolmentToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("EnrolmentToken(<redacted>)")
    }
}

impl EnrolmentToken {
    /// The token's text, to hand to the person once.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// What an authenticator app is given: the seed in base32 to type in, and the URI to scan.
#[derive(Debug)]
pub struct Provisioning {
    /// The seed, base32.
    pub secret: String,
    /// The `otpauth://` URI.
    pub uri: String,
}

/// How completing an enrolment ended.
#[derive(Debug)]
pub enum Completion {
    /// Enrolled; the session that signs the person in.
    Enrolled(SessionToken),
    /// No such link: unknown, used, expired, or never begun. One answer for all four.
    Unknown,
    /// The code is not the pending seed's.
    WrongCode,
    /// The password breaks the policy; nothing was written and the link still works.
    WeakPassword(PasswordError),
}

/// Enrolment links and the credentials they create.
pub struct EnrolmentService {
    database: Database,
    place: Place,
    accounts: Arc<AccountService>,
    passwords: Arc<Passwords>,
    seal: Arc<TotpSeal>,
    sessions: Arc<crate::sessions::SessionService>,
}

impl EnrolmentService {
    pub(crate) fn new(
        database: Database,
        place: Place,
        accounts: Arc<AccountService>,
        passwords: Arc<Passwords>,
        seal: Arc<TotpSeal>,
        sessions: Arc<crate::sessions::SessionService>,
    ) -> Self {
        Self {
            database,
            place,
            accounts,
            passwords,
            seal,
            sessions,
        }
    }

    /// A new link for `account`, replacing any earlier one. The caller has decided it may be issued.
    ///
    /// # Errors
    ///
    /// [`Error::Conflict`] when the account already has a credential; [`Error::Store`] when the store fails.
    pub async fn system_issue(&self, account: &AccountId) -> Result<EnrolmentToken> {
        let token = hex(&random::bytes::<ENROLMENT_TOKEN_BYTES>()?);
        let key = digest_key(&token).ok_or(Error::Unreadable {
            record: "enrolment",
            reason: "a fresh token did not have a token's shape".to_owned(),
        })?;
        let lifetime = Value::Duration {
            seconds: i64::from(ENROLMENT_LIFETIME_HOURS).saturating_mul(3600),
            nanos: 0,
        };
        let pointer = format!("of-{}", account.as_str());
        // The link this one replaces is named by the pointer; the transaction re-reads it and refuses if another issue
        // moved it meanwhile, so two racing issues cannot both leave a live link. No real key is the word `none`.
        let earlier = self
            .database
            .run_at(
                &self.place,
                "GET enrolments:$pointer;",
                vec![bind("pointer", pointer.as_str())],
            )
            .await?;
        let old = field(earlier.into_iter().next(), "key").unwrap_or_else(|| "none".to_owned());
        self.database
            .run_at(
                &self.place,
                "BEGIN; \
                 LET $held = SELECT account_id FROM credentials WHERE account_id = $account LIMIT 1; \
                 THROW 'already enrolled' IF array::len($held) > 0; \
                 LET $now = GET enrolments:$pointer; \
                 THROW 'standing changed' IF ($now.key ?? 'none') != $old; \
                 DELETE enrolments:$old; \
                 SET enrolments:$key = { account: $account } EXPIRE $lifetime; \
                 SET enrolments:$pointer = { key: $key } EXPIRE $lifetime; \
                 COMMIT;",
                vec![
                    bind("account", account.as_str()),
                    bind("key", key.as_str()),
                    bind("pointer", pointer.as_str()),
                    bind("old", old.as_str()),
                    ("lifetime".to_owned(), lifetime),
                ],
            )
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(EnrolmentToken(token))
    }

    /// Begin enrolling with `token`: a fresh TOTP seed for the person's authenticator app, kept sealed beside the
    /// link until they confirm it. `None` for a token that is unknown, used or expired.
    ///
    /// # Errors
    ///
    /// When the store, the randomness or the seal fails; never for a bad token.
    pub async fn begin(&self, token: &str) -> Result<Option<Provisioning>> {
        let Some(key) = digest_key(token) else {
            return Ok(None);
        };
        let Some(account) = self.account_of(&key).await? else {
            return Ok(None);
        };
        let Some(found) = self.accounts.get(&account).await? else {
            return Ok(None);
        };
        let seed = random::bytes::<TOTP_SECRET_BYTES>()?;
        let factor = hex(&random::bytes::<16>()?);
        let sealed = hex(&self.seal.seal(account.as_str(), &factor, &seed)?);
        let rewritten = self
            .database
            .run_at(
                &self.place,
                "BEGIN; LET $held = GET enrolments:$key; \
                 THROW 'not found' IF $held = NONE OR $held.account != $account; \
                 SET enrolments:$key = { account: $account, factor: $factor, seed: $sealed } \
                 EXPIRE TTL enrolments:$key; COMMIT;",
                vec![
                    bind("key", key.as_str()),
                    bind("account", account.as_str()),
                    bind("factor", factor.as_str()),
                    bind("sealed", sealed.as_str()),
                ],
            )
            .await
            .map_err(|error| thrown(error.into()));
        match rewritten {
            Ok(_) => {}
            Err(Error::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        }
        let (secret, uri) = totp::provisioning(ISSUER, found.email.as_str(), &seed);
        Ok(Some(Provisioning { secret, uri }))
    }

    /// Complete enrolling with `token`: `code` from the authenticator app proves the seed arrived, `password` becomes
    /// the account's, the link is consumed, and the person is signed in.
    ///
    /// # Errors
    ///
    /// [`Error::Conflict`] when the account was enrolled meanwhile; when the store or the custody fails.
    pub async fn complete(&self, token: &str, password: &str, code: &str) -> Result<Completion> {
        let Some(key) = digest_key(token) else {
            return Ok(Completion::Unknown);
        };
        let answers = self
            .database
            .run_at(
                &self.place,
                "GET enrolments:$key;",
                vec![bind("key", key.as_str())],
            )
            .await?;
        let held = answers.into_iter().next();
        let (Some(account), Some(factor), Some(sealed)) = (
            field(held.clone(), "account"),
            field(held.clone(), "factor"),
            field(held, "seed").and_then(|text| unhex(&text)),
        ) else {
            return Ok(Completion::Unknown);
        };
        let Ok(account) = AccountId::parse(&account) else {
            return Ok(Completion::Unknown);
        };
        let Some(found) = self.accounts.get(&account).await? else {
            return Ok(Completion::Unknown);
        };
        let password = match Password::parse(password, &found.email) {
            Ok(password) => password,
            Err(weak) => return Ok(Completion::WeakPassword(weak)),
        };
        let seed = self.seal.open(account.as_str(), &factor, &sealed)?;
        let Ok(step) = totp::verify(&seed, code, now_secs(), None) else {
            return Ok(Completion::WrongCode);
        };
        let verifier = self.passwords.hash(password.expose()).await?;
        let sealed = hex(&sealed);
        let step = i64::try_from(step).map_err(|_| Error::Unreadable {
            record: "enrolment",
            reason: "a TOTP step beyond the integer range".to_owned(),
        })?;
        let pointer = format!("of-{}", account.as_str());
        let written = self
            .database
            .run_at(
                &self.place,
                "BEGIN; LET $held = GET enrolments:$key; \
                 THROW 'not found' IF $held = NONE OR $held.factor != $factor; \
                 LET $enrolled = SELECT account_id FROM credentials WHERE account_id = $account LIMIT 1; \
                 THROW 'already enrolled' IF array::len($enrolled) > 0; \
                 CREATE credentials:$account = { account_id: $account, verifier: $verifier, totp: $sealed, \
                 totp_factor: $factor, totp_last_step: $step, created: time::now() }; \
                 DELETE enrolments:$key; DELETE enrolments:$pointer; COMMIT;",
                vec![
                    bind("key", key.as_str()),
                    bind("factor", factor.as_str()),
                    bind("account", account.as_str()),
                    bind("verifier", verifier.as_str()),
                    bind("sealed", sealed.as_str()),
                    bind("step", step),
                    bind("pointer", pointer.as_str()),
                ],
            )
            .await
            .map_err(|error| thrown(error.into()));
        match written {
            Ok(_) => Ok(Completion::Enrolled(self.sessions.issue(&account).await?)),
            Err(Error::NotFound) => Ok(Completion::Unknown),
            Err(error) => Err(error),
        }
    }

    async fn account_of(&self, key: &str) -> Result<Option<AccountId>> {
        let answers = self
            .database
            .run_at(&self.place, "GET enrolments:$key;", vec![bind("key", key)])
            .await?;
        Ok(field(answers.into_iter().next(), "account").and_then(|id| AccountId::parse(&id).ok()))
    }
}
