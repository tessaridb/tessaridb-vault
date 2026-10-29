//! Identities at an OpenID Connect issuer, and the requests in flight to one.
//!
//! An identity is `(issuer, subject)`, never an address: it is linked to an account only by that account, signed
//! in, asking for it, and an identity linked once cannot be linked to a second account. A sign-in through the issuer
//! finds the account by the identity alone. A request in flight is kept under the digest of its `state` for ten
//! minutes and is consumed by its first return, whichever return that is. A return completes only for the party
//! that began the request: a sign-in for the browser holding the binding issued at its start, a link for the same
//! signed-in account; anyone else holding the return address completes nothing.

use std::sync::Arc;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq as _;
use tessari_vault_constants::SESSION_TOKEN_BYTES;
use tessari_vault_infrastructure::oidc::Identity;
use tessari_vault_infrastructure::random;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::{AccountId, AccountState};
use tessaridb_oauth_client::relying::Pending;

use crate::access::bind;
use crate::accounts::AccountService;
use crate::enrolments::record::field;
use crate::sessions::{SessionService, SessionToken, digest_key, hex};
use crate::{Conflict, Error, Result};

/// How long a request to the issuer waits for its return, in seconds.
const PENDING_SECONDS: i64 = 600;

/// Identities and requests in flight.
pub struct IdentityService {
    database: Database,
    place: Place,
    accounts: Arc<AccountService>,
    sessions: Arc<SessionService>,
}

fn key_of(state: &str) -> String {
    hex(&Sha256::digest(state.as_bytes()))
}

fn unreadable(reason: &str) -> Error {
    Error::Unreadable {
        record: "oidc request",
        reason: reason.to_owned(),
    }
}

/// A kept request as it reads back: the request, `sign-in` or `link`, and the binding digest or the account.
struct Kept {
    pending: Pending,
    purpose: String,
    holder: String,
}

impl IdentityService {
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

    /// Keep `pending` as a sign-in until its return, and the binding the browser must present with that return.
    ///
    /// # Errors
    ///
    /// When randomness is unavailable or the store fails.
    pub async fn keep_sign_in(&self, pending: &Pending) -> Result<String> {
        let binding = hex(&random::bytes::<SESSION_TOKEN_BYTES>()?);
        let digest = digest_key(&binding).ok_or_else(|| unreadable("a fresh binding"))?;
        self.keep(pending, "sign-in", &digest).await?;
        Ok(binding)
    }

    /// Keep `pending` as a request to link the identity it proves to `account`.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn keep_link(&self, pending: &Pending, account: &AccountId) -> Result<()> {
        self.keep(pending, "link", account.as_str()).await
    }

    async fn keep(&self, pending: &Pending, purpose: &str, holder: &str) -> Result<()> {
        let kept = serde_json::to_string(pending).map_err(|_| unreadable("did not serialize"))?;
        self.database
            .run_at(
                &self.place,
                "SET oidc_pending:$key = { pending: $pending, purpose: $purpose, holder: $holder } \
                 EXPIRE $lifetime;",
                vec![
                    bind("key", key_of(pending.state()).as_str()),
                    bind("pending", kept.as_str()),
                    bind("purpose", purpose),
                    bind("holder", holder),
                    (
                        "lifetime".to_owned(),
                        Value::Duration {
                            seconds: PENDING_SECONDS,
                            nanos: 0,
                        },
                    ),
                ],
            )
            .await?;
        Ok(())
    }

    /// The sign-in kept under `state`, when the browser presents the `binding` issued at its start. The request is
    /// consumed either way.
    ///
    /// # Errors
    ///
    /// When the store fails or the kept request does not read back.
    pub async fn take_sign_in(&self, state: &str, binding: &str) -> Result<Option<Pending>> {
        let Some(kept) = self.take(state).await? else {
            return Ok(None);
        };
        let presented = digest_key(binding).unwrap_or_default();
        let held = kept.purpose == "sign-in"
            && bool::from(presented.as_bytes().ct_eq(kept.holder.as_bytes()));
        Ok(held.then_some(kept.pending))
    }

    /// The link request kept under `state`, when `account` is the one that asked for it. The request is consumed
    /// either way.
    ///
    /// # Errors
    ///
    /// When the store fails or the kept request does not read back.
    pub async fn take_link(&self, state: &str, account: &AccountId) -> Result<Option<Pending>> {
        let Some(kept) = self.take(state).await? else {
            return Ok(None);
        };
        let held = kept.purpose == "link" && kept.holder == account.as_str();
        Ok(held.then_some(kept.pending))
    }

    /// The request kept under `state`, removed: a second call for the same `state` finds nothing.
    async fn take(&self, state: &str) -> Result<Option<Kept>> {
        let key = key_of(state);
        let answers = self
            .database
            .read_at(
                &self.place,
                "GET oidc_pending:$key;",
                vec![bind("key", key.as_str())],
            )
            .await?;
        let held = answers.into_iter().next();
        let (Some(kept), Some(purpose), Some(holder)) = (
            field(held.clone(), "pending"),
            field(held.clone(), "purpose"),
            field(held, "holder"),
        ) else {
            return Ok(None);
        };
        let taken = self
            .database
            .transact_at(
                &self.place,
                "BEGIN; LET $held = GET oidc_pending:$key; THROW 'not found' IF $held = NONE; \
                 DELETE oidc_pending:$key; COMMIT;",
                "",
                vec![bind("key", key.as_str())],
            )
            .await
            .map_err(|error| crate::access::thrown(error.into()));
        match taken {
            Ok(_) => {}
            Err(Error::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        }
        let pending = serde_json::from_str(&kept).map_err(|_| unreadable("did not read back"))?;
        Ok(Some(Kept {
            pending,
            purpose,
            holder,
        }))
    }

    /// A session for the active account `identity` is linked to; `None` when it is linked to none, or the account
    /// cannot sign in.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn sign_in(&self, identity: &Identity) -> Result<Option<SessionToken>> {
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT account FROM identities WHERE issuer = $issuer AND subject = $subject LIMIT 1;",
                vec![
                    bind("issuer", identity.issuer.as_str()),
                    bind("subject", identity.subject.as_str()),
                ],
            )
            .await?;
        let account = crate::answers::records::<LinkedAccount>(answers, "identity")?
            .pop()
            .and_then(|linked| AccountId::parse(&linked.account).ok());
        let Some(account) = account else {
            return Ok(None);
        };
        match self.accounts.get(&account).await? {
            Some(found) if found.state == AccountState::Active => {
                Ok(Some(self.sessions.issue(&account).await?))
            }
            _ => Ok(None),
        }
    }

    /// Link `identity` to `account`.
    ///
    /// # Errors
    ///
    /// [`Error::Conflict`] when the identity is already linked, to this account or another.
    pub async fn link(&self, account: &AccountId, identity: &Identity) -> Result<()> {
        let linked = self
            .database
            .run_at(
                &self.place,
                "CREATE identities = { issuer: $issuer, subject: $subject, account: $account, linked: time::now() };",
                vec![
                    bind("issuer", identity.issuer.as_str()),
                    bind("subject", identity.subject.as_str()),
                    bind("account", account.as_str()),
                ],
            )
            .await;
        match linked {
            Ok(_) => Ok(()),
            Err(tessari_vault_infrastructure::Error::Refused { message, .. })
                if message.starts_with("unique index by_identity") =>
            {
                Err(Error::Conflict(Conflict::IdentityLinked))
            }
            Err(error) => Err(error.into()),
        }
    }
}

/// The `account` column of an `identities` row.
struct LinkedAccount {
    account: String,
}

impl tessari_vault_infrastructure::tessaridb::FromRecord for LinkedAccount {
    fn from_row(
        mut row: tessari_vault_infrastructure::tessaridb::Row,
    ) -> std::result::Result<Self, tessari_vault_infrastructure::tessaridb::MappingFault> {
        Ok(Self {
            account: row.take("account")?,
        })
    }
}
