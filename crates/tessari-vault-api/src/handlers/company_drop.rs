//! Deleting a company: an owner asks with a statement signed in the browser by their own registered key.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use tessari_vault_storage::company_drops::DropState;

use super::company_id;
use super::records::decoded;
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Signed {
    /// The time the statement names, in seconds since 1970.
    at: u64,
    /// The stored signature over the statement, base64url.
    signature: String,
}

#[derive(Serialize)]
pub(crate) struct Standing {
    state: &'static str,
    /// When a confirmed deletion may be carried out, in seconds since 1970.
    #[serde(skip_serializing_if = "Option::is_none")]
    drop_after: Option<i64>,
}

impl From<DropState> for Standing {
    fn from(state: DropState) -> Self {
        Self {
            state: state.word(),
            drop_after: match state {
                DropState::Scheduled { drop_after } => Some(drop_after),
                DropState::Requested | DropState::Cancelled => None,
            },
        }
    }
}

pub(crate) async fn request(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<Signed>,
) -> Result<(StatusCode, Json<Standing>), ApiError> {
    let company = company_id(&company)?;
    let signature = decoded(&body.signature)?;
    let standing = state
        .storage
        .company_drops
        .request(&actor, &company, body.at, &signature)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(standing.into())))
}

pub(crate) async fn confirm(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Standing>, ApiError> {
    let company = company_id(&company)?;
    let standing = state
        .storage
        .company_drops
        .confirm(&actor, &company)
        .await?;
    Ok(Json(standing.into()))
}

pub(crate) async fn cancel(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<Signed>,
) -> Result<Json<Standing>, ApiError> {
    let company = company_id(&company)?;
    let signature = decoded(&body.signature)?;
    let standing = state
        .storage
        .company_drops
        .cancel(&actor, &company, body.at, &signature)
        .await?;
    Ok(Json(standing.into()))
}
