//! Sessions: a bearer token held by the browser, and under it in the store only its SHA-256 digest.
//!
//! The token is 256 random bits, so the digest needs no salt and no slow hash: nobody can guess a token to test
//! against it. A digest that leaks from a backup cannot be presented, because the server hashes what it is given.

use std::fmt;

use sha2::{Digest, Sha256};
use tessari_vault_constants::{SESSION_LIFETIME_MINUTES, SESSION_TOKEN_BYTES};
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Answer, Database, Place, Value};
use tessari_vault_types::{AccountId, AccountState};

use crate::access::{Actor, bind};
use crate::accounts::AccountService;
use crate::{Error, Result};

/// A session token as the browser holds it; never printed.
pub struct SessionToken(String);

impl SessionToken {
    /// The token's text, to hand to the browser once.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken(<redacted>)")
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The store key for a token's text, or `None` when the text is not the shape a token has.
pub(crate) fn digest_key(token: &str) -> Option<String> {
    let shaped = token.len() == SESSION_TOKEN_BYTES.saturating_mul(2)
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    shaped.then(|| hex(&Sha256::digest(token.as_bytes())))
}

/// Sessions.
pub struct SessionService {
    database: Database,
    place: Place,
    accounts: std::sync::Arc<AccountService>,
}

impl SessionService {
    pub(crate) fn new(
        database: Database,
        place: Place,
        accounts: std::sync::Arc<AccountService>,
    ) -> Self {
        Self {
            database,
            place,
            accounts,
        }
    }

    /// Open a session for `account`, which sign-in has verified.
    ///
    /// # Errors
    ///
    /// When randomness is unavailable or the store fails.
    pub async fn issue(&self, account: &AccountId) -> Result<SessionToken> {
        let token = hex(&random::bytes::<SESSION_TOKEN_BYTES>()?);
        let key = digest_key(&token).ok_or(Error::Unreadable {
            record: "session",
            reason: "a fresh token did not have a token's shape".to_owned(),
        })?;
        let lifetime = Value::Duration {
            seconds: i64::from(SESSION_LIFETIME_MINUTES).saturating_mul(60),
            nanos: 0,
        };
        self.database
            .run_at(
                &self.place,
                "SET sessions:$key = { account: $account } EXPIRE $lifetime;",
                vec![
                    bind("key", key.as_str()),
                    bind("account", account.as_str()),
                    ("lifetime".to_owned(), lifetime),
                ],
            )
            .await?;
        Ok(SessionToken(token))
    }

    /// End the session `token` names; a token of the wrong shape or an unknown one ends nothing.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn end(&self, token: &str) -> Result<()> {
        if let Some(key) = digest_key(token) {
            self.database
                .run_at(
                    &self.place,
                    "DELETE sessions:$key;",
                    vec![bind("key", key.as_str())],
                )
                .await?;
        }
        Ok(())
    }

    /// The caller a token belongs to: `None` for a token of the wrong shape, an unknown or expired one, or one
    /// whose account is gone or suspended. It is one answer, so a caller cannot tell which.
    ///
    /// # Errors
    ///
    /// When the store fails; never for a bad token.
    pub async fn resolve(&self, token: &str) -> Result<Option<Actor>> {
        let Some(key) = digest_key(token) else {
            return Ok(None);
        };
        let answers = self
            .database
            .run_at(
                &self.place,
                "GET sessions:$key;",
                vec![bind("key", key.as_str())],
            )
            .await?;
        let account = match answers.into_iter().next() {
            Some(Answer::Value {
                value: Value::Object(session),
                ..
            }) => match session.get("account") {
                Some(Value::String(id)) => AccountId::parse(id).ok(),
                _ => None,
            },
            _ => None,
        };
        let Some(account) = account else {
            return Ok(None);
        };
        Ok(self
            .accounts
            .get(&account)
            .await?
            .filter(|found| found.state == AccountState::Active)
            .map(|found| Actor::new(found.id, found.platform_admin)))
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
