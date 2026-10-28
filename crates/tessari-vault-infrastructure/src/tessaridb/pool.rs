//! A bounded set of signed-in connections, and the one way work reaches the store through them.
//!
//! A connection is a session: it holds the identity it signed in with and whatever `USE` it was last given. The
//! identity is worth keeping (a password costs an Argon2id verification, so it is spent once per connection)
//! and the `USE` is not, so every unit of work carries its own. A connection that failed is dropped rather than
//! handed to the next caller in an unknown state, and so is a new connection whose sign-in may not have happened:
//! reused, it would send work with no credentials and no session behind it.
//!
//! The node verifies only so many passwords at once and answers `throttled` beyond that, so new connections sign
//! in a few at a time and a throttled sign-in is tried again, bounded.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tessari_vault_constants::{
    STORE_RETRY_FIRST_PAUSE_MS, STORE_RETRY_MAX_PAUSE_MS, STORE_SIGNIN_ATTEMPTS,
    STORE_SIGNIN_CONCURRENCY,
};
use tessaridb_client::{Answer, Client, RefusalClass, Request, Tls, Value, VaultKey};
use tokio::sync::{Semaphore, SemaphorePermit};

use super::connection::{Connection, bound};
use super::{Place, StoreConfig};
use crate::{Error, Result};

/// A handle on the store; cheap to clone, shared by every caller.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
}

struct Inner {
    config: StoreConfig,
    tls: Option<Tls>,
    idle: Mutex<Vec<Connection>>,
    slots: Semaphore,
    signins: Semaphore,
}

impl Database {
    /// A pool of at most `connections` connections to the store `config` names. Nothing connects until used.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] when the configuration would send the password in the clear across a network, or
    /// names an authority that is not a certificate.
    pub fn new(config: StoreConfig, connections: usize) -> Result<Self> {
        let tls = match &config.authority {
            Some(pem) => Some(Tls::trusting_pem(pem).map_err(|_| Error::InvalidConfig {
                field: "store_authority",
                reason: "the authority must be one or more PEM certificates",
            })?),
            None if is_loopback_literal(&config.address) => None,
            None => {
                return Err(Error::InvalidConfig {
                    field: "store_authority",
                    reason: "without an authority the store is reached only at 127.0.0.1 or [::1]",
                });
            }
        };
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                tls,
                idle: Mutex::new(Vec::new()),
                slots: Semaphore::new(connections),
                signins: Semaphore::new(STORE_SIGNIN_CONCURRENCY),
            }),
        })
    }

    /// Run `script` at `place` with its values bound, answering one [`Answer`] per statement of `script`.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] with the node's own words, or [`Error::Unavailable`] when the store cannot be reached.
    pub async fn run_at(
        &self,
        place: &Place,
        script: &str,
        parameters: Vec<(String, Value)>,
    ) -> Result<Vec<Answer>> {
        let preamble = place.preamble();
        self.exchange(&bound(format!("{preamble} {script}"), parameters), 2)
            .await
    }

    /// As [`Database::run_at`], with `key` presented for the keyed vault `vault` at `place` for this request alone:
    /// the node opens the vault with it and keeps nothing of it afterwards.
    ///
    /// # Errors
    ///
    /// As [`Database::run_at`]; the node refuses a key that does not open the vault.
    pub async fn run_presenting(
        &self,
        place: &Place,
        script: &str,
        parameters: Vec<(String, Value)>,
        vault: &str,
        key: VaultKey,
    ) -> Result<Vec<Answer>> {
        let preamble = place.preamble();
        let request = bound(format!("{preamble} {script}"), parameters)
            .presenting((place.namespace(), place.database(), vault), key);
        self.exchange(&request, 2).await
    }

    /// Declare the vault `vault` at `place` in key custody, its key wrapped under `key`, which is never stored.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] when `vault` is not a plain name; [`Error::Refused`] when the node refuses, a name
    /// already taken included; [`Error::Unavailable`] when the store cannot be reached.
    pub async fn declare_keyed_vault(
        &self,
        place: &Place,
        vault: &str,
        key: &VaultKey,
    ) -> Result<()> {
        let (_slot, mut client, _) = self.converse(&Request::new(place.preamble())).await?;
        match client
            .declare_keyed((place.namespace(), place.database()), vault, key)
            .await
        {
            Ok(()) => {
                self.release(client);
                Ok(())
            }
            Err(tessaridb_client::Error::Build(_)) => {
                self.release(client);
                Err(Error::InvalidName {
                    name: vault.to_owned(),
                })
            }
            Err(failure) => Err(self.failed(client, false, failure)),
        }
    }

    /// Run `script` with no place selected, for provisioning, whose statements name their own places.
    ///
    /// # Errors
    ///
    /// As [`Database::run_at`].
    pub async fn run_unplaced(
        &self,
        script: &str,
        parameters: Vec<(String, Value)>,
    ) -> Result<Vec<Answer>> {
        self.exchange(&bound(script.to_owned(), parameters), 0)
            .await
    }

    async fn exchange(&self, request: &Request, preamble: usize) -> Result<Vec<Answer>> {
        let (_slot, client, answers) = self.converse(request).await?;
        self.release(client);
        Ok(answers.into_iter().skip(preamble).collect())
    }

    /// Send `request` on a connection, signing a new one in with it, and hand the connection back with the
    /// answers so the caller can send more on the same session before releasing it.
    async fn converse(
        &self,
        request: &Request,
    ) -> Result<(SemaphorePermit<'_>, Connection, Vec<Answer>)> {
        let slot = self
            .inner
            .slots
            .acquire()
            .await
            .map_err(|_| Error::Unavailable {
                message: "the connection pool is closed".to_owned(),
            })?;
        let reused = self
            .inner
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        if let Some(client) = reused {
            let (client, answers) = self.send(client, false, request).await?;
            return Ok((slot, client, answers));
        }
        let mut pause = Duration::from_millis(STORE_RETRY_FIRST_PAUSE_MS);
        let longest = Duration::from_millis(STORE_RETRY_MAX_PAUSE_MS);
        let mut made = 1;
        loop {
            let signing = self
                .inner
                .signins
                .acquire()
                .await
                .map_err(|_| Error::Unavailable {
                    message: "the connection pool is closed".to_owned(),
                })?;
            let client = self.connect().await?;
            let outcome = self.send(client, true, request).await;
            drop(signing);
            let throttled = matches!(
                &outcome,
                Err(Error::Refused {
                    class: Some(RefusalClass::Throttled),
                    ..
                })
            );
            if !throttled || made >= STORE_SIGNIN_ATTEMPTS {
                return outcome.map(|(client, answers)| (slot, client, answers));
            }
            tokio::time::sleep(pause).await;
            pause = pause.saturating_mul(2).min(longest);
            made = made.saturating_add(1);
        }
    }

    /// Send one request on `client`, signing in first when `signing_in`; on a refusal the connection is kept only
    /// when its session is certainly intact.
    async fn send(
        &self,
        mut client: Connection,
        signing_in: bool,
        request: &Request,
    ) -> Result<(Connection, Vec<Answer>)> {
        let config = &self.inner.config;
        let signed = signing_in.then(|| request.clone().as_user(&config.user, &config.password));
        match client.send(signed.as_ref().unwrap_or(request)).await {
            Ok(answers) => Ok((client, answers)),
            Err(failure) => Err(self.failed(client, signing_in, failure)),
        }
    }

    /// The error a failed exchange on `client` reports, keeping the connection only when its session is intact.
    fn failed(
        &self,
        client: Connection,
        signing_in: bool,
        failure: tessaridb_client::Error,
    ) -> Error {
        match failure {
            tessaridb_client::Error::Refused { message, class } => {
                // A refused statement leaves the session intact, unless the refusal was about the session itself,
                // or was a sign-in the node did not take, which leaves a connection that never signed in.
                let session_lost = class == Some(RefusalClass::Unauthenticated)
                    || (signing_in && class == Some(RefusalClass::Throttled));
                if !session_lost {
                    self.release(client);
                }
                Error::Refused { message, class }
            }
            failure => Error::Unavailable {
                message: failure.to_string(),
            },
        }
    }

    async fn connect(&self) -> Result<Connection> {
        let address = &self.inner.config.address;
        let connected = match &self.inner.tls {
            Some(tls) => Client::connect_tls(address, tls)
                .await
                .map(|client| Connection::Secured(Box::new(client))),
            None => Client::connect(address.as_str())
                .await
                .map(Connection::Plain),
        };
        let client = connected.map_err(|failure| Error::Unavailable {
            message: failure.to_string(),
        })?;
        tracing::debug!(%address, "store connection opened");
        Ok(client)
    }

    fn release(&self, client: Connection) {
        self.inner
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(client);
    }
}

/// Whether `address` is a loopback IP literal with a port: the only place the clear is accepted, because nothing
/// sent there leaves the machine. A name is not accepted even when it is `localhost`: a name can resolve elsewhere.
fn is_loopback_literal(address: &str) -> bool {
    address
        .parse::<SocketAddr>()
        .is_ok_and(|socket| socket.ip().is_loopback())
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
