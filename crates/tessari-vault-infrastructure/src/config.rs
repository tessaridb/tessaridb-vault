//! The node's settings, read from the environment once at start-up.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::tessaridb::StoreConfig;
use crate::{Error, Result};

/// Everything the node needs to start.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// How to reach the store.
    pub store: StoreConfig,
    /// The store namespace the server keeps everything in.
    pub namespace: String,
    /// The database inside it.
    pub database: String,
    /// Where the HTTP API listens.
    pub listen: SocketAddr,
    /// The address and name of the platform admin to ensure at start-up, when set.
    pub platform_admin: Option<(String, String)>,
    /// The file holding the service key (K_svc), read once at start-up.
    pub service_key_file: PathBuf,
    /// The OpenID Connect issuer people may sign in through, when one is configured.
    pub oidc: Option<OidcConfig>,
}

/// An OpenID Connect issuer and this server's registration there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    /// The issuer identifier, discovered at start.
    pub issuer: String,
    /// This server's client id at the issuer.
    pub client_id: String,
    /// The redirect URI registered there.
    pub redirect: String,
}

impl NodeConfig {
    /// The settings the variables `lookup` answers hold.
    ///
    /// `TESSARIDB_VAULT_STORE` (wire `host:port`), `TESSARIDB_VAULT_STORE_USER`, `TESSARIDB_VAULT_STORE_PASSWORD` and
    /// `TESSARIDB_VAULT_SERVICE_KEY_FILE` are required; `TESSARIDB_VAULT_STORE_AUTHORITY` is a PEM file for a store reached over TLS;
    /// `TESSARIDB_VAULT_NAMESPACE` (default `vault`), `TESSARIDB_VAULT_DATABASE` (default `main`),
    /// `TESSARIDB_VAULT_LISTEN` (default `127.0.0.1:8080`); `TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL` with
    /// `TESSARIDB_VAULT_PLATFORM_ADMIN_NAME`, both or neither; `TESSARIDB_VAULT_OIDC_ISSUER`,
    /// `TESSARIDB_VAULT_OIDC_CLIENT_ID` and `TESSARIDB_VAULT_OIDC_REDIRECT`, all three or none.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] naming the first variable that is missing or malformed.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let required = |key: &'static str| {
            lookup(key)
                .filter(|value| !value.is_empty())
                .ok_or(Error::InvalidConfig {
                    field: key,
                    reason: "required",
                })
        };
        let authority = match lookup("TESSARIDB_VAULT_STORE_AUTHORITY") {
            Some(path) if !path.is_empty() => {
                Some(std::fs::read(path).map_err(|_| Error::InvalidConfig {
                    field: "TESSARIDB_VAULT_STORE_AUTHORITY",
                    reason: "a readable PEM file",
                })?)
            }
            _ => None,
        };
        let listen = lookup("TESSARIDB_VAULT_LISTEN")
            .unwrap_or_else(|| "127.0.0.1:8080".to_owned())
            .parse()
            .map_err(|_| Error::InvalidConfig {
                field: "TESSARIDB_VAULT_LISTEN",
                reason: "an address of the form host:port",
            })?;
        let platform_admin = match (
            lookup("TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL"),
            lookup("TESSARIDB_VAULT_PLATFORM_ADMIN_NAME"),
        ) {
            (Some(email), Some(name)) => Some((email, name)),
            (None, None) => None,
            _ => {
                return Err(Error::InvalidConfig {
                    field: "TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL",
                    reason: "set together with TESSARIDB_VAULT_PLATFORM_ADMIN_NAME, or neither",
                });
            }
        };
        let oidc = match (
            lookup("TESSARIDB_VAULT_OIDC_ISSUER"),
            lookup("TESSARIDB_VAULT_OIDC_CLIENT_ID"),
            lookup("TESSARIDB_VAULT_OIDC_REDIRECT"),
        ) {
            (Some(issuer), Some(client_id), Some(redirect)) => Some(OidcConfig {
                issuer,
                client_id,
                redirect,
            }),
            (None, None, None) => None,
            _ => {
                return Err(Error::InvalidConfig {
                    field: "TESSARIDB_VAULT_OIDC_ISSUER",
                    reason: "set together with TESSARIDB_VAULT_OIDC_CLIENT_ID and TESSARIDB_VAULT_OIDC_REDIRECT, or none",
                });
            }
        };
        Ok(Self {
            store: StoreConfig {
                address: required("TESSARIDB_VAULT_STORE")?,
                user: required("TESSARIDB_VAULT_STORE_USER")?,
                password: required("TESSARIDB_VAULT_STORE_PASSWORD")?,
                authority,
            },
            namespace: lookup("TESSARIDB_VAULT_NAMESPACE").unwrap_or_else(|| "vault".to_owned()),
            database: lookup("TESSARIDB_VAULT_DATABASE").unwrap_or_else(|| "main".to_owned()),
            listen,
            platform_admin,
            service_key_file: required("TESSARIDB_VAULT_SERVICE_KEY_FILE")?.into(),
            oidc,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
