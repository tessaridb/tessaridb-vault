//! Own sign-in: an address and a password, then a code from the authenticator app, then a session.
//!
//! The password step answers one way for everything that is not a known, active, enrolled account with the right
//! password, after a verification of full cost either way, so neither the answer nor its timing says whether an
//! address has an account. A right password yields a short-lived single-use challenge, kept only as its digest; the
//! code step accepts each TOTP step once, recorded by compare-and-set on the credential. Attempts are counted under
//! keyed digests of the address and of the address-and-password pair, and per network address; over a count the
//! answer is a back-off, never a lockout of the account.

mod outcome;

use std::sync::Arc;

use tessari_vault_constants::{
    SESSION_TOKEN_BYTES, SIGN_IN_CHALLENGE_MINUTES, SIGN_IN_CODE_MISSES, SIGN_IN_PER_ACCOUNT,
    SIGN_IN_PER_ADDRESS, SIGN_IN_PER_PAIR,
};
use tessari_vault_core::totp;
use tessari_vault_infrastructure::custody::{Passwords, SigninKeys, TotpSeal, Verdict};
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value};
use tessari_vault_types::{AccountId, AccountState, Email};

use crate::access::bind;
use crate::accounts::AccountService;
use crate::answers::single;
use crate::enrolments::record::{now_secs, unhex};
use crate::sessions::{SessionService, digest_key, hex};
use crate::throttle::ThrottleService;
use crate::{Error, Result};
use outcome::CredentialEntity;
pub use outcome::{Challenge, CodeStep, PasswordStep};

/// Own sign-in.
pub struct SignInService {
    database: Database,
    place: Place,
    accounts: Arc<AccountService>,
    passwords: Arc<Passwords>,
    seal: Arc<TotpSeal>,
    keys: SigninKeys,
    sessions: Arc<SessionService>,
    throttle: Arc<ThrottleService>,
}

/// The parts the sign-in service is made of.
pub(crate) struct SignInParts {
    pub(crate) accounts: Arc<AccountService>,
    pub(crate) passwords: Arc<Passwords>,
    pub(crate) seal: Arc<TotpSeal>,
    pub(crate) keys: SigninKeys,
    pub(crate) sessions: Arc<SessionService>,
    pub(crate) throttle: Arc<ThrottleService>,
}

impl SignInService {
    pub(crate) fn new(database: Database, place: Place, parts: SignInParts) -> Self {
        Self {
            database,
            place,
            accounts: parts.accounts,
            passwords: parts.passwords,
            seal: parts.seal,
            keys: parts.keys,
            sessions: parts.sessions,
            throttle: parts.throttle,
        }
    }

    /// The password step for `email` and `password`, sent from network address `source`.
    ///
    /// # Errors
    ///
    /// When the store, the randomness or the verifier fails; never for a wrong password.
    pub async fn password(
        &self,
        email: &Email,
        password: &str,
        source: &str,
    ) -> Result<PasswordStep> {
        for (key, limit) in [
            (
                format!("sign-in-account:{}", self.keys.account(email.as_str())),
                SIGN_IN_PER_ACCOUNT,
            ),
            (
                format!("sign-in-pair:{}", self.keys.pair(email.as_str(), password)),
                SIGN_IN_PER_PAIR,
            ),
            (format!("sign-in-address:{source}"), SIGN_IN_PER_ADDRESS),
        ] {
            if !self.throttle.allow(&key, limit).await? {
                return Ok(PasswordStep::Throttled);
            }
        }
        let account = self
            .accounts
            .find_by_email(email)
            .await?
            .filter(|found| found.state == AccountState::Active);
        let credential = match &account {
            Some(found) => self.credential(&found.id).await?,
            None => None,
        };
        let stored = match &credential {
            Some(held) => Some((held.epoch()?, held.verifier.as_str())),
            None => None,
        };
        let verdict = self.passwords.verify(password, stored).await?;
        let (Some(found), Verdict::Match { rehash }) = (account, verdict) else {
            return Ok(PasswordStep::Failed);
        };
        if rehash {
            let verifier = self.passwords.hash(password).await?;
            self.database
                .run_at(
                    &self.place,
                    "UPDATE credentials:$account SET verifier = $verifier, key_epoch = $key_epoch;",
                    vec![
                        bind("account", found.id.as_str()),
                        bind("verifier", verifier.phc.as_str()),
                        bind("key_epoch", i64::from(verifier.epoch)),
                    ],
                )
                .await?;
        }
        let challenge = hex(&random::bytes::<SESSION_TOKEN_BYTES>()?);
        let key = digest_key(&challenge).ok_or(Error::Unreadable {
            record: "sign-in",
            reason: "a fresh challenge did not have a token's shape".to_owned(),
        })?;
        let lifetime = Value::Duration {
            seconds: i64::from(SIGN_IN_CHALLENGE_MINUTES).saturating_mul(60),
            nanos: 0,
        };
        self.database
            .run_at(
                &self.place,
                "SET sign_ins:$key = { account: $account } EXPIRE $lifetime;",
                vec![
                    bind("key", key.as_str()),
                    bind("account", found.id.as_str()),
                    ("lifetime".to_owned(), lifetime),
                ],
            )
            .await?;
        Ok(PasswordStep::Challenge(Challenge::new(challenge)))
    }

    /// The code step: `code` for the challenge `challenge`.
    ///
    /// # Errors
    ///
    /// When the store or the seal fails; never for a wrong code or a dead challenge.
    pub async fn code(&self, challenge: &str, code: &str) -> Result<CodeStep> {
        let Some(key) = digest_key(challenge) else {
            return Ok(CodeStep::Unknown);
        };
        let answers = self
            .database
            .read_at(
                &self.place,
                "GET sign_ins:$key;",
                vec![bind("key", key.as_str())],
            )
            .await?;
        let Some(account) = account_of(answers.into_iter().next()) else {
            return Ok(CodeStep::Unknown);
        };
        let Some(credential) = self.credential(&account).await? else {
            return Ok(CodeStep::Unknown);
        };
        let Some(sealed) = unhex(&credential.totp) else {
            return Err(Error::Unreadable {
                record: "credential",
                reason: "a sealed seed that is not hex".to_owned(),
            });
        };
        let seed = self
            .seal
            .open(account.as_str(), &credential.factor, &sealed)?;
        let last = u64::try_from(credential.last_step).ok();
        let Ok(step) = totp::verify(&seed, code, now_secs(), last) else {
            return self.missed(&key).await;
        };
        let step = i64::try_from(step).map_err(|_| Error::Unreadable {
            record: "credential",
            reason: "a TOTP step beyond the integer range".to_owned(),
        })?;
        // The step is recorded by compare-and-set: of two requests carrying one code, exactly one moves it.
        let recorded = self
            .database
            .transact_at(
                &self.place,
                "BEGIN; UPDATE credentials:$account SET totp_last_step = $step WHERE totp_last_step < $step; \
                 DELETE sign_ins:$key; COMMIT;",
                "",
                vec![
                    bind("account", account.as_str()),
                    bind("step", step),
                    bind("key", key.as_str()),
                ],
            )
            .await;
        match recorded {
            Ok(_) => {}
            Err(tessari_vault_infrastructure::Error::Refused { .. }) => {
                return self.missed(&key).await;
            }
            Err(error) => return Err(error.into()),
        }
        match self.accounts.get(&account).await? {
            Some(found) if found.state == AccountState::Active => {
                Ok(CodeStep::Signed(self.sessions.issue(&account).await?))
            }
            _ => Ok(CodeStep::Unknown),
        }
    }

    /// Count a wrong code against the challenge, ending it at the limit.
    async fn missed(&self, key: &str) -> Result<CodeStep> {
        if !self
            .throttle
            .allow(
                &format!("sign-in-misses:{key}"),
                SIGN_IN_CODE_MISSES.saturating_sub(1),
            )
            .await?
        {
            self.database
                .run_at(&self.place, "DELETE sign_ins:$key;", vec![bind("key", key)])
                .await?;
        }
        Ok(CodeStep::WrongCode)
    }

    async fn credential(&self, account: &AccountId) -> Result<Option<CredentialEntity>> {
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT verifier, key_epoch, totp, totp_factor, totp_last_step FROM credentials WHERE account_id = $account LIMIT 1;",
                vec![bind("account", account.as_str())],
            )
            .await?;
        single(answers, "credential")
    }
}

fn account_of(answer: Option<Answer>) -> Option<AccountId> {
    match answer {
        Some(Answer::Value {
            value: Value::Object(object),
            ..
        }) => match object.get("account") {
            Some(Value::String(id)) => AccountId::parse(id).ok(),
            _ => None,
        },
        _ => None,
    }
}
