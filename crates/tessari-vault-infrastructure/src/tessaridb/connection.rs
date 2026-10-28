//! One session to the store, over TLS or in the clear to a loopback address, and the two things sent on it: a
//! request, and the vault frame's act that declares a vault in key custody.

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

    /// Declare `vault` in `within` with its key wrapped under `key`; refused by the node when the name is taken.
    pub(super) async fn declare_keyed(
        &mut self,
        within: (&str, &str),
        vault: &str,
        key: &VaultKey,
    ) -> Result<()> {
        match self {
            Self::Plain(client) => Vault::new(client, within, vault)
                .map_err(Error::Build)?
                .declare_keyed(key)
                .await
                .map(drop),
            Self::Secured(client) => Vault::new(client.as_mut(), within, vault)
                .map_err(Error::Build)?
                .declare_keyed(key)
                .await
                .map(drop),
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
