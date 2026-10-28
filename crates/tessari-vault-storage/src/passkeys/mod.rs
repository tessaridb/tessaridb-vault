//! Passkeys: an account's registered credentials, and the ceremonies in flight that register or use one.
//!
//! A ceremony is a fresh challenge kept under its digest for five minutes and consumed by its first answer. A
//! registration belongs to the signed-in account that began it; a sign-in begins with no account at all, so nothing
//! about the request says whether an account exists, and the credential the authenticator answers with names the
//! account. Both ask for user verification: at sign-in a passkey is the whole credential. A credential id is
//! registered once; its counter only moves forward, and a counter that did not is refused as a possible clone.

mod ceremony;
mod record;

use std::sync::Arc;

use tessari_vault_core::webauthn::{self, Expected};
use tessari_vault_infrastructure::config::PasskeyParty;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_types::{AccountId, AccountState, base64url};

use crate::access::bind;
use crate::accounts::AccountService;
use crate::sessions::{SessionService, SessionToken, hex};
use crate::{Conflict, Error, Result};
use record::{Stored, StoredId};

/// An assertion as the browser sends it, every part still undecoded base64url.
pub struct Assertion<'a> {
    /// The credential id the authenticator chose.
    pub credential_id: &'a str,
    /// The client data JSON.
    pub client_data_json: &'a str,
    /// The authenticator data.
    pub authenticator_data: &'a str,
    /// The signature over both.
    pub signature: &'a str,
    /// The user handle the credential was registered with: the account id.
    pub user_handle: &'a str,
}

/// Passkeys and their ceremonies.
pub struct PasskeyService {
    database: Database,
    place: Place,
    accounts: Arc<AccountService>,
    sessions: Arc<SessionService>,
}

fn decoded(text: &str) -> Option<Vec<u8>> {
    base64url::decode(text).ok()
}

impl PasskeyService {
    pub(crate) fn new(
        database: Database,
        place: Place,
        accounts: Arc<AccountService>,
        sessions: Arc<SessionService>,
    ) -> Self {
        Self {
            database,
            place,
            accounts,
            sessions,
        }
    }

    /// Begin registering a passkey for `account`: the challenge, base64url.
    ///
    /// # Errors
    ///
    /// When randomness is unavailable or the store fails.
    pub async fn begin_registration(&self, account: &AccountId) -> Result<String> {
        self.begin("register", account.as_str()).await
    }

    /// Begin a sign-in with a passkey: the challenge, base64url.
    ///
    /// # Errors
    ///
    /// When randomness is unavailable or the store fails.
    pub async fn begin_sign_in(&self) -> Result<String> {
        self.begin("sign-in", "").await
    }

    /// Register the passkey the response proves, for `account`, which began the ceremony: its credential id, or
    /// `None` when the response is refused.
    ///
    /// # Errors
    ///
    /// [`Conflict::PasskeyRegistered`] when the credential is registered already; otherwise when the store fails.
    pub async fn register(
        &self,
        account: &AccountId,
        party: &PasskeyParty,
        client_data_json: &str,
        attestation_object: &str,
    ) -> Result<Option<String>> {
        let (Some(client_data), Some(attestation)) =
            (decoded(client_data_json), decoded(attestation_object))
        else {
            return Ok(None);
        };
        let Some((purpose, holder, challenge)) = self.take(&client_data).await? else {
            return Ok(None);
        };
        if purpose != "register" || holder != account.as_str() {
            return Ok(None);
        }
        let origins = [party.origin.as_str()];
        let expected = Expected {
            rp_id: &party.rp_id,
            origins: &origins,
            challenge: &challenge,
            user_verification: true,
        };
        let Ok(registered) = webauthn::register(&client_data, &attestation, &expected) else {
            return Ok(None);
        };
        let credential = base64url::encode(&registered.credential_id);
        let created = self
            .database
            .run_at(
                &self.place,
                "CREATE passkeys = { account: $account, credential: $credential, public_key: $key, \
                 sign_count: $count, backup_eligible: $backup, created: time::now() };",
                vec![
                    bind("account", account.as_str()),
                    bind("credential", credential.as_str()),
                    bind("key", hex(&registered.public_key).as_str()),
                    bind("count", i64::from(registered.sign_count)),
                    bind("backup", registered.backup_eligible),
                ],
            )
            .await;
        match created {
            Ok(_) => Ok(Some(credential)),
            Err(tessari_vault_infrastructure::Error::Refused { message, .. })
                if message.starts_with("unique index by_credential") =>
            {
                Err(Error::Conflict(Conflict::PasskeyRegistered))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The credential ids `account` has registered, base64url: what a new registration excludes.
    ///
    /// # Errors
    ///
    /// When the store fails or a row does not read back.
    pub async fn credentials_of(&self, account: &AccountId) -> Result<Vec<String>> {
        let answers = self
            .database
            .run_at(
                &self.place,
                "SELECT credential FROM passkeys WHERE account = $account;",
                vec![bind("account", account.as_str())],
            )
            .await?;
        Ok(crate::answers::records::<StoredId>(answers, "passkey")?
            .into_iter()
            .map(|row| row.credential)
            .collect())
    }

    /// A session for the account the assertion proves, or `None` when it proves nothing: an unknown or spent
    /// ceremony, an unknown credential, a user handle of another account, any failed check, a counter that did not
    /// move forward, or an account that cannot sign in.
    ///
    /// # Errors
    ///
    /// When the store fails or a stored passkey does not read back.
    pub async fn sign_in(
        &self,
        party: &PasskeyParty,
        assertion: &Assertion<'_>,
    ) -> Result<Option<SessionToken>> {
        let (Some(client_data), Some(data), Some(signature)) = (
            decoded(assertion.client_data_json),
            decoded(assertion.authenticator_data),
            decoded(assertion.signature),
        ) else {
            return Ok(None);
        };
        let Some((purpose, _, challenge)) = self.take(&client_data).await? else {
            return Ok(None);
        };
        let Some(stored) = self.stored(assertion.credential_id).await? else {
            return Ok(None);
        };
        if purpose != "sign-in"
            || decoded(assertion.user_handle).as_deref() != Some(stored.account.as_bytes())
        {
            return Ok(None);
        }
        let origins = [party.origin.as_str()];
        let expected = Expected {
            rp_id: &party.rp_id,
            origins: &origins,
            challenge: &challenge,
            user_verification: true,
        };
        let public_key = record::unhex(&stored.public_key).ok_or_else(|| Error::Unreadable {
            record: "passkey",
            reason: "a stored key that is not hex".to_owned(),
        })?;
        let count = u32::try_from(stored.sign_count).unwrap_or(u32::MAX);
        let Ok(asserted) = webauthn::assert(
            &client_data,
            &data,
            &signature,
            &public_key,
            count,
            &expected,
        ) else {
            return Ok(None);
        };
        if asserted.clone_suspected
            || !self
                .moved(assertion.credential_id, count, asserted.sign_count)
                .await?
        {
            return Ok(None);
        }
        let Ok(account) = AccountId::parse(&stored.account) else {
            return Ok(None);
        };
        match self.accounts.get(&account).await? {
            Some(found) if found.state == AccountState::Active => {
                Ok(Some(self.sessions.issue(&account).await?))
            }
            _ => Ok(None),
        }
    }

    async fn stored(&self, credential: &str) -> Result<Option<Stored>> {
        let answers = self
            .database
            .run_at(
                &self.place,
                "SELECT account, public_key, sign_count FROM passkeys WHERE credential = $credential LIMIT 1;",
                vec![bind("credential", credential)],
            )
            .await?;
        Ok(crate::answers::records::<Stored>(answers, "passkey")?.pop())
    }

    /// Move the credential's counter from `from` to `to` by compare-and-set; `false` when another sign-in moved it
    /// first. A credential that keeps no counter (both zero) has nothing to move.
    async fn moved(&self, credential: &str, from: u32, to: u32) -> Result<bool> {
        if from == 0 && to == 0 {
            return Ok(true);
        }
        let answers = self
            .database
            .run_at(
                &self.place,
                "UPDATE passkeys SET sign_count = $to WHERE credential = $credential AND sign_count = $from \
                 RETURN AFTER;",
                vec![
                    bind("credential", credential),
                    bind("from", i64::from(from)),
                    bind("to", i64::from(to)),
                ],
            )
            .await?;
        Ok(!crate::answers::records::<StoredId>(answers, "passkey")?.is_empty())
    }
}
