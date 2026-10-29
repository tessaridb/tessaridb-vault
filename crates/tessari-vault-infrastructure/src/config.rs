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
    /// The passkey relying party, from the address people reach the server at, when one is configured.
    pub passkeys: Option<PasskeyParty>,
}

/// The relying party passkeys are scoped to: the host people reach the server at, and that origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyParty {
    /// The relying-party id: the host.
    pub rp_id: String,
    /// The one origin a browser response may name.
    pub origin: String,
}

/// The relying party of `public_url`: https, or http to `localhost` or a loopback address; a bare origin with no path or user.
fn passkey_party(public_url: &str) -> Result<PasskeyParty> {
    let refuse = |reason| Error::InvalidConfig {
        field: "TESSARIDB_VAULT_PUBLIC_URL",
        reason,
    };
    let url = url::Url::parse(public_url).map_err(|_| refuse("an absolute URL"))?;
    let host = url.host_str().ok_or(refuse("a URL with a host"))?;
    // `localhost` counts too: it is the only plain-http origin a browser both trusts and accepts as a passkey's
    // relying-party id, which an IP address never is.
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback)) {
        return Err(refuse("https, or http to localhost or a loopback address"));
    }
    if url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
    {
        return Err(refuse("an origin with no path, query, fragment or user"));
    }
    Ok(PasskeyParty {
        rp_id: host.to_owned(),
        origin: url.origin().ascii_serialization(),
    })
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

/// Where the server listens: `TESSARIDB_VAULT_LISTEN`, default `127.0.0.1:8080`. Read alone by `health`, which needs
/// nothing else.
///
/// # Errors
///
/// [`Error::InvalidConfig`] for an address that is not `host:port`.
pub fn listen_address(lookup: impl Fn(&str) -> Option<String>) -> Result<SocketAddr> {
    lookup("TESSARIDB_VAULT_LISTEN")
        .unwrap_or_else(|| "127.0.0.1:8080".to_owned())
        .parse()
        .map_err(|_| Error::InvalidConfig {
            field: "TESSARIDB_VAULT_LISTEN",
            reason: "an address of the form host:port",
        })
}

impl NodeConfig {
    /// The settings the variables `lookup` answers hold.
    ///
    /// `TESSARIDB_VAULT_STORE` (wire `host:port`), `TESSARIDB_VAULT_STORE_USER`, `TESSARIDB_VAULT_STORE_PASSWORD` and
    /// `TESSARIDB_VAULT_SERVICE_KEY_FILE` are required; `TESSARIDB_VAULT_STORE_AUTHORITY` is a PEM file for a store reached over TLS;
    /// `TESSARIDB_VAULT_NAMESPACE` (default `vault`), `TESSARIDB_VAULT_DATABASE` (default `main`),
    /// `TESSARIDB_VAULT_LISTEN` (default `127.0.0.1:8080`); `TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL` with
    /// `TESSARIDB_VAULT_PLATFORM_ADMIN_NAME`, both or neither; `TESSARIDB_VAULT_OIDC_ISSUER`,
    /// `TESSARIDB_VAULT_OIDC_CLIENT_ID` and `TESSARIDB_VAULT_OIDC_REDIRECT`, all three or none;
    /// `TESSARIDB_VAULT_PUBLIC_URL`, the address people reach the server at, for passkeys.
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
        let listen = listen_address(&lookup)?;
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
        let passkeys = lookup("TESSARIDB_VAULT_PUBLIC_URL")
            .map(|url| passkey_party(&url))
            .transpose()?;
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
            passkeys,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
