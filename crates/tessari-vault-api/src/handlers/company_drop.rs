//! Deleting a company: an owner asks with a statement signed in the browser by their own registered key.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

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
    Ok((
        StatusCode::ACCEPTED,
        Json(Standing {
            state: standing.word(),
        }),
    ))
}
