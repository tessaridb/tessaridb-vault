//! Sign-in through an OpenID Connect issuer: TessariDB OAuth, or any issuer found by discovery.
//!
//! At start the issuer's endpoints are discovered once. A sign-in begins with a request carrying PKCE (S256), a
//! `state` and a `nonce`, which the caller keeps; the return is accepted only with that `state` and this issuer in
//! `iss`, the code is exchanged with the PKCE verifier, and the ID token is checked against the issuer's key set,
//! this client as its audience, its time, and the request's `nonce`. What comes out is the identity the issuer
//! vouches for, `(issuer, subject)`; which account that is, is the caller's question.

use std::time::{SystemTime, UNIX_EPOCH};

pub use tessaridb_oauth_client::relying::Pending;
use tessaridb_oauth_client::relying::{self, Client, TokenEndpoint};
use tessaridb_oauth_client::resource::{Algorithm, Expectations, HttpsKeySource, KeyCache, key_id};

use crate::{Error, Result};

/// The scope a sign-in asks for: the identity and nothing else.
const SCOPE: &str = "openid";

/// One configured issuer.
pub struct OidcProvider {
    issuer: String,
    client: Client,
    tokens: TokenEndpoint,
    keys: KeyCache<HttpsKeySource>,
    expect: Expectations,
}

impl std::fmt::Debug for OidcProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OidcProvider")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

/// The identity an issuer vouched for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The issuer, exactly as configured.
    pub issuer: String,
    /// The user's subject at that issuer.
    pub subject: String,
}

/// Why a return did not give an identity; never more detail than the caller needs to refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReturnFailed {
    /// Not this request's return: another `state`, another issuer, a denial, or malformed.
    #[error("the return is not this request's")]
    NotThisRequest,
    /// The code could not be exchanged.
    #[error("the issuer did not exchange the code")]
    Exchange,
    /// No ID token, or one that did not pass every check.
    #[error("the ID token was not accepted")]
    IdToken,
}

const fn refuse(reason: &'static str) -> Error {
    Error::InvalidConfig {
        field: "TESSARIDB_VAULT_OIDC_ISSUER",
        reason,
    }
}

impl OidcProvider {
    /// Discover `issuer` and prepare sign-ins for `client_id`, returning to `redirect`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] naming `TESSARIDB_VAULT_OIDC_ISSUER` when discovery fails or what it found cannot be
    /// used.
    pub async fn discover(issuer: &str, client_id: &str, redirect: &str) -> Result<Self> {
        let metadata = relying::discover(issuer)
            .await
            .map_err(|_| refuse("the issuer could not be discovered"))?;
        let client = Client::new(
            &metadata.issuer,
            &metadata.authorization_endpoint,
            client_id,
            redirect,
        )
        .map_err(|_| refuse("the issuer's endpoints, client id or redirect cannot be used"))?;
        let tokens = TokenEndpoint::new(&metadata.token_endpoint)
            .map_err(|_| refuse("the issuer's token endpoint cannot be used"))?;
        let keys = KeyCache::new(
            HttpsKeySource::new(&metadata.jwks_uri)
                .map_err(|_| refuse("the issuer's key set address cannot be used"))?,
        );
        let expect = Expectations::new(
            metadata.issuer.clone(),
            client_id,
            &[Algorithm::Es256, Algorithm::EdDsa],
        )
        .map_err(|_| refuse("no expectations could be made"))?;
        Ok(Self {
            issuer: metadata.issuer,
            client,
            tokens,
            keys,
            expect,
        })
    }

    /// The address to send the user to, and the request to keep under its `state` until they return.
    ///
    /// # Errors
    ///
    /// [`Error::Randomness`] when no `state`, `nonce` or verifier can be drawn.
    pub fn begin(&self) -> Result<(String, Pending)> {
        let (address, pending) =
            relying::begin(&self.client, SCOPE, None).map_err(|_| Error::Randomness)?;
        Ok((address.to_string(), pending))
    }

    /// The identity the return `query` of `pending` proves.
    ///
    /// # Errors
    ///
    /// [`ReturnFailed`] naming the step that refused.
    pub async fn finish(
        &self,
        pending: &Pending,
        query: &str,
    ) -> std::result::Result<Identity, ReturnFailed> {
        let code = pending
            .accept(query)
            .map_err(|_| ReturnFailed::NotThisRequest)?;
        let tokens = self
            .tokens
            .exchange(&self.client, pending, &code, None, None)
            .await
            .map_err(|_| ReturnFailed::Exchange)?;
        let id_token = tokens.id_token().ok_or(ReturnFailed::IdToken)?;
        let kid = key_id(id_token).map_err(|_| ReturnFailed::IdToken)?;
        let keys = self
            .keys
            .keys_for(&kid)
            .await
            .map_err(|_| ReturnFailed::IdToken)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let verified =
            relying::validate_id_token(id_token, &keys, &self.expect, pending.nonce(), now)
                .map_err(|_| ReturnFailed::IdToken)?;
        Ok(Identity {
            issuer: self.issuer.clone(),
            subject: verified.subject,
        })
    }
}
