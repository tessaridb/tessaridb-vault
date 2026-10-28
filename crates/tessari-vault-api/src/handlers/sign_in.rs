//! Own sign-in (address and password, then a code) and signing out.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::signin::{CodeStep, PasswordStep};
use tessari_vault_types::Email;
use zeroize::Zeroizing;

use crate::ApiError;
use crate::caller::{Body, Caller, Unsigned};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Credentials {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub(crate) struct Challenge {
    challenge: String,
}

pub(crate) async fn password(
    State(state): State<AppState>,
    Unsigned(source): Unsigned,
    Body(body): Body<Credentials>,
) -> Result<Json<Challenge>, ApiError> {
    let password = Zeroizing::new(body.password);
    // An address that is not one cannot have an account; it gets the answer every failed sign-in gets.
    let Ok(email) = Email::parse(&body.email) else {
        return Err(ApiError::SIGN_IN_FAILED);
    };
    match state
        .storage
        .sign_in
        .password(&email, &password, &source)
        .await?
    {
        PasswordStep::Challenge(challenge) => Ok(Json(Challenge {
            challenge: challenge.expose().to_owned(),
        })),
        PasswordStep::Failed => Err(ApiError::SIGN_IN_FAILED),
        PasswordStep::Throttled => Err(ApiError::THROTTLED),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Code {
    challenge: String,
    code: String,
}

#[derive(Serialize)]
pub(crate) struct Session {
    session: String,
}

pub(crate) async fn code(
    State(state): State<AppState>,
    _: Unsigned,
    Body(body): Body<Code>,
) -> Result<(StatusCode, Json<Session>), ApiError> {
    let challenge = Zeroizing::new(body.challenge);
    match state.storage.sign_in.code(&challenge, &body.code).await? {
        CodeStep::Signed(session) => Ok((
            StatusCode::CREATED,
            Json(Session {
                session: session.expose().to_owned(),
            }),
        )),
        CodeStep::Unknown => Err(ApiError::SIGN_IN_FAILED),
        CodeStep::WrongCode => Err(ApiError::WRONG_CODE),
    }
}

pub(crate) async fn sign_out(
    State(state): State<AppState>,
    Caller(_): Caller,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    // The extractor proved the bearer token names a live session; this ends exactly that one.
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::UNAUTHENTICATED)?;
    state.storage.sessions.end(token).await?;
    Ok(StatusCode::NO_CONTENT)
}
