//! Enrolment: an admin issues a member's link; the member, with no session yet, sets their own password and TOTP.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::enrolments::Completion;
use zeroize::Zeroizing;

use super::{account_id, company_id};
use crate::ApiError;
use crate::caller::{Body, Caller, Unsigned};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct Link {
    token: String,
}

pub(crate) async fn issue(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, account)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Link>), ApiError> {
    let company = company_id(&company)?;
    let account = account_id(&account)?;
    let token = state
        .storage
        .companies
        .issue_enrolment(&actor, &company, &account)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(Link {
            token: token.expose().to_owned(),
        }),
    ))
}

/// The platform admin resets the sign-in of an account whose password rests on a retired service-key epoch, and
/// receives the link that enrols them again.
pub(crate) async fn reset(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(account): Path<String>,
) -> Result<(StatusCode, Json<Link>), ApiError> {
    let account = account_id(&account)?;
    let token = state
        .storage
        .enrolments
        .reset_retired(&actor, &account)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(Link {
            token: token.expose().to_owned(),
        }),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Begin {
    token: String,
}

#[derive(Serialize)]
pub(crate) struct Authenticator {
    secret: String,
    uri: String,
}

pub(crate) async fn begin(
    State(state): State<AppState>,
    _: Unsigned,
    Body(body): Body<Begin>,
) -> Result<Json<Authenticator>, ApiError> {
    let token = Zeroizing::new(body.token);
    let provisioning = state
        .storage
        .enrolments
        .begin(&token)
        .await?
        .ok_or(ApiError::NOT_FOUND)?;
    Ok(Json(Authenticator {
        secret: provisioning.secret,
        uri: provisioning.uri,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Complete {
    token: String,
    password: String,
    code: String,
}

#[derive(Serialize)]
pub(crate) struct Session {
    session: String,
}

pub(crate) async fn complete(
    State(state): State<AppState>,
    _: Unsigned,
    Body(body): Body<Complete>,
) -> Result<(StatusCode, Json<Session>), ApiError> {
    let token = Zeroizing::new(body.token);
    let password = Zeroizing::new(body.password);
    match state
        .storage
        .enrolments
        .complete(&token, &password, &body.code)
        .await?
    {
        Completion::Enrolled(session) => Ok((
            StatusCode::CREATED,
            Json(Session {
                session: session.expose().to_owned(),
            }),
        )),
        Completion::Unknown => Err(ApiError::NOT_FOUND),
        Completion::WrongCode => Err(ApiError::WRONG_CODE),
        Completion::WeakPassword(weak) => Err(ApiError::weak_password(weak)),
    }
}
