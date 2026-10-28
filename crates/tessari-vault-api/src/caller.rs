//! The signed-in caller, taken from the bearer token and nothing else, and the request counters that bound them.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequest, FromRequestParts, Request};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use tessari_vault_constants::{RATE_PER_ACCOUNT, RATE_PER_ADDRESS};
use tessari_vault_storage::access::Actor;

use crate::ApiError;
use crate::state::AppState;

/// The caller a request's bearer token belongs to. A handler that takes one cannot run for an unsigned request.
pub struct Caller(pub Actor);

impl FromRequestParts<AppState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // The address bucket is counted before anything else, so a caller without a session is bounded too.
        count_address(parts, state).await?;
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::UNAUTHENTICATED)?;
        let actor = state
            .storage
            .sessions
            .resolve(token)
            .await
            .map_err(ApiError::from)?
            .ok_or(ApiError::UNAUTHENTICATED)?;
        within(
            state,
            &format!("account:{}", actor.account()),
            RATE_PER_ACCOUNT,
        )
        .await?;
        Ok(Self(actor))
    }
}

/// A request with no session, counted against its network address, which it carries: the unsigned routes take this
/// instead of [`Caller`].
pub struct Unsigned(pub String);

impl FromRequestParts<AppState> for Unsigned {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(count_address(parts, state).await?))
    }
}

async fn count_address(parts: &Parts, state: &AppState) -> Result<String, ApiError> {
    let address = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(
            || "unknown".to_owned(),
            |ConnectInfo(peer)| peer.ip().to_string(),
        );
    within(state, &format!("address:{address}"), RATE_PER_ADDRESS).await?;
    Ok(address)
}

/// Count one request against `key`. A store that cannot count refuses the request: an unbounded endpoint is the
/// wrong way to fail.
async fn within(state: &AppState, key: &str, limit: u32) -> Result<(), ApiError> {
    if state
        .storage
        .throttle
        .allow(key, limit)
        .await
        .map_err(ApiError::from)?
    {
        Ok(())
    } else {
        Err(ApiError::THROTTLED)
    }
}

/// A JSON body, refused with the API's own error shape rather than the framework's.
pub struct Body<T>(pub T);

impl<T: DeserializeOwned> FromRequest<AppState> for Body<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<T>::from_request(request, state)
            .await
            .map_err(|_| ApiError::INVALID)?;
        Ok(Self(value))
    }
}
