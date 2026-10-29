//! Sign-in through the configured OpenID Connect issuer, and linking an identity there to the signed-in account.
//!
//! The issuer sends the browser back to the panel, which hands the return query to one route. It completes the sign-in
//! or the link kept under the return's `state`, only for the browser holding the binding cookie set when it began: a
//! link's return carries no session, since the page that asked for it was left for the issuer. Every failure of a
//! return answers as a failed sign-in.

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::header::{COOKIE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tessari_vault_infrastructure::oidc::{OidcProvider, Pending};
use tessari_vault_storage::identities::Returned;

use crate::ApiError;
use crate::caller::{Caller, Unsigned};
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

/// The cookie holding `binding` until the return, for this browser alone.
fn bound(binding: &str) -> String {
    format!("{BINDING}={binding}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=600")
}

/// The cookie that clears the binding once a return has spent it.
fn cleared() -> String {
    format!("{BINDING}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")
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
        [(LOCATION, address), (SET_COOKIE, bound(&binding))],
    )
        .into_response())
}

/// The return: a session for the account a signed-in identity is linked to, or the identity linked to the account
/// that asked.
pub(crate) async fn returned(
    State(state): State<AppState>,
    _: Unsigned,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let provider = provider(&state)?;
    let query = query.unwrap_or_default();
    let binding = binding(&headers).unwrap_or_default();
    let taken = state
        .storage
        .identities
        .take_return(&state_of(&query)?, &binding)
        .await?
        .ok_or(ApiError::SIGN_IN_FAILED)?;
    let (pending, linking) = match taken {
        Returned::SignIn(pending) => (pending, None),
        Returned::Link(pending, account) => (pending, Some(account)),
    };
    let identity = provider
        .finish(&pending, &query)
        .await
        .map_err(|_| ApiError::SIGN_IN_FAILED)?;
    if let Some(account) = linking {
        state.storage.identities.link(&account, &identity).await?;
        return Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, cleared())]).into_response());
    }
    let session = state
        .storage
        .identities
        .sign_in(&identity)
        .await?
        .ok_or(ApiError::SIGN_IN_FAILED)?;
    Ok((
        StatusCode::CREATED,
        [(SET_COOKIE, cleared())],
        Json(Session {
            session: session.expose().to_owned(),
        }),
    )
        .into_response())
}

/// Begin linking an identity at the issuer to the caller's account: the address to send the browser to, and the
/// binding its return must come back with.
pub(crate) async fn link(
    State(state): State<AppState>,
    Caller(actor): Caller,
) -> Result<Response, ApiError> {
    let (address, pending) = begun(provider(&state)?)?;
    let binding = state
        .storage
        .identities
        .keep_link(&pending, actor.account())
        .await?;
    Ok(([(SET_COOKIE, bound(&binding))], Json(Address { address })).into_response())
}
