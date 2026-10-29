//! One session to the store, over TLS or in the clear to a loopback address, and the two things sent on it: a
//! request, and the vault frame's acts on a vault in key custody.

use tessaridb_client::{Answer, Client, Error, Request, Result, Secured, Value, Vault, VaultKey};

/// One signed-in session, over TLS, or in the clear to a loopback address only.
pub(super) enum Connection {
    Plain(Client),
    /// Boxed: a TLS session carries its whole record layer, many times the size of a plain one.
    Secured(Box<Client<Secured>>),
}

impl Connection {
    pub(super) async fn send(&mut self, request: &Request) -> Result<Vec<Answer>> {
        match self {
            Self::Plain(client) => client.send(request).await,
            Self::Secured(client) => client.send(request).await,
        }
    }

    /// Send `act` to `vault` in `within`.
    pub(super) async fn keyed(
        &mut self,
        within: (&str, &str),
        vault: &str,
        act: KeyAct<'_>,
    ) -> Result<()> {
        match self {
            Self::Plain(client) => {
                act.send(Vault::new(client, within, vault).map_err(Error::Build)?)
                    .await
            }
            Self::Secured(client) => {
                act.send(Vault::new(client.as_mut(), within, vault).map_err(Error::Build)?)
                    .await
            }
        }
    }
}

/// An act on a vault in key custody: declaring it under a key, or re-wrapping its key from one key to another.
#[derive(Clone, Copy)]
pub(super) enum KeyAct<'a> {
    Declare(&'a VaultKey),
    Change {
        current: &'a VaultKey,
        new: &'a VaultKey,
    },
}

impl KeyAct<'_> {
    async fn send<S>(self, mut vault: Vault<'_, S>) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        match self {
            Self::Declare(key) => vault.declare_keyed(key).await.map(drop),
            Self::Change { current, new } => vault.change_key(current, new).await.map(drop),
        }
    }
}

/// A request to run `script` with `parameters` bound by name.
pub(super) fn bound(script: String, parameters: Vec<(String, Value)>) -> Request {
    parameters
        .into_iter()
        .fold(Request::new(script), |request, (name, value)| {
            request.bind(name, value)
        })
}
