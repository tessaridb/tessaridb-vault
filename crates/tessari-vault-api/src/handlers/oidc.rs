//! Sign-in through the configured OpenID Connect issuer, and linking an identity there to the signed-in account.
//!
//! The issuer sends the browser back to the panel, which hands the return query to one of two routes: the sign-in
//! return, which needs the binding cookie set when that browser began, or the link return, which needs the session of
//! the account that asked for the link. Every failure of a return answers as a failed sign-in.

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::header::{COOKIE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tessari_vault_infrastructure::oidc::{OidcProvider, Pending};

use crate::ApiError;
use crate::caller::{Body, Caller, Unsigned};
use crate::state::AppState;

/// The cookie binding a sign-in's return to the browser that began it.
const BINDING: &str = "__Host-vault-oidc";

#[derive(Serialize)]
pub(crate) struct Session {
    session: String,
}

#[derive(Serialize)]
pub(crate) struct Address {
    address: String,
}

#[derive(Deserialize)]
pub(crate) struct LinkReturn {
    query: String,
}

fn provider(state: &AppState) -> Result<&OidcProvider, ApiError> {
    state.oidc.as_deref().ok_or(ApiError::NOT_FOUND)
}

fn begun(provider: &OidcProvider) -> Result<(String, Pending), ApiError> {
    provider
        .begin()
        .map_err(|error| ApiError::from(tessari_vault_storage::Error::from(error)))
}

/// The `state` a return query carries.
fn state_of(query: &str) -> Result<String, ApiError> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .ok_or(ApiError::SIGN_IN_FAILED)
}

/// The binding cookie's value, when the request carries one.
fn binding(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|pair| pair.trim().strip_prefix(BINDING)?.strip_prefix('='))
        .map(str::to_owned)
}

/// Begin a sign-in: send the browser to the issuer, holding a binding only it will present on return.
pub(crate) async fn start(
    State(state): State<AppState>,
    _: Unsigned,
) -> Result<Response, ApiError> {
    let (address, pending) = begun(provider(&state)?)?;
    let binding = state.storage.identities.keep_sign_in(&pending).await?;
    Ok((
        StatusCode::SEE_OTHER,
        [
            (LOCATION, address),
            (
                SET_COOKIE,
                format!("{BINDING}={binding}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=600"),
            ),
        ],
    )
        .into_response())
}

/// The sign-in return: a session for the account the identity is linked to.
pub(crate) async fn returned(
    State(state): State<AppState>,
    _: Unsigned,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let provider = provider(&state)?;
    let query = query.unwrap_or_default();
    let binding = binding(&headers).unwrap_or_default();
    let pending = state
        .storage
        .identities
        .take_sign_in(&state_of(&query)?, &binding)
        .await?
        .ok_or(ApiError::SIGN_IN_FAILED)?;
    let identity = provider
        .finish(&pending, &query)
        .await
        .map_err(|_| ApiError::SIGN_IN_FAILED)?;
    let session = state
        .storage
        .identities
        .sign_in(&identity)
        .await?
        .ok_or(ApiError::SIGN_IN_FAILED)?;
    Ok((
        StatusCode::CREATED,
        [(
            SET_COOKIE,
            format!("{BINDING}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"),
        )],
        Json(Session {
            session: session.expose().to_owned(),
        }),
    )
        .into_response())
}

/// Begin linking an identity at the issuer to the caller's account: the address to send the browser to.
pub(crate) async fn link(
    State(state): State<AppState>,
    Caller(actor): Caller,
) -> Result<Json<Address>, ApiError> {
    let (address, pending) = begun(provider(&state)?)?;
    state
        .storage
        .identities
        .keep_link(&pending, actor.account())
        .await?;
    Ok(Json(Address { address }))
}

/// The link return, sent by the account that asked for the link: the identity is linked to it.
pub(crate) async fn link_returned(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Body(body): Body<LinkReturn>,
) -> Result<StatusCode, ApiError> {
    let provider = provider(&state)?;
    let pending = state
        .storage
        .identities
        .take_link(&state_of(&body.query)?, actor.account())
        .await?
        .ok_or(ApiError::SIGN_IN_FAILED)?;
    let identity = provider
        .finish(&pending, &body.query)
        .await
        .map_err(|_| ApiError::SIGN_IN_FAILED)?;
    state
        .storage
        .identities
        .link(actor.account(), &identity)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
