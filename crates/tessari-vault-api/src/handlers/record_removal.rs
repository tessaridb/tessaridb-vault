//! Deleting a record: its author or a company admin, with the reason the caller gives on the trail.

use axum::extract::{Path, State};
use axum::http::StatusCode;

use super::company_id;
use super::records::record_id;
use crate::ApiError;
use crate::caller::Caller;
use crate::state::AppState;

pub(crate) async fn delete(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    state
        .storage
        .records
        .delete(&actor, &company, &record)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
