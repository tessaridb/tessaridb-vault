//! The company recovery key (C7): owners keep it, wrapped to owners only; every member reads its public half, and an
//! owner their own wrap of it.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;
use tessari_vault_storage::group_keys::NewGroupKey;
use tessari_vault_types::base64url;

use super::company_id;
use super::group_keys::{GroupKeyBody, decoded, key_id, wraps};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct RecoveryKeyItem {
    key_id: String,
    public: String,
}

#[derive(Serialize)]
pub(crate) struct OwnWrap {
    key_id: String,
    wrap: String,
}

pub(crate) async fn keep(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<GroupKeyBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let key = NewGroupKey::new(
        key_id(&body.key_id)?,
        decoded(&body.public)?,
        wraps(body.wraps)?,
    )
    .map_err(|_| ApiError::INVALID)?;
    state.storage.recovery.keep(&actor, &company, key).await?;
    Ok(StatusCode::CREATED)
}

pub(crate) async fn read(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<RecoveryKeyItem>, ApiError> {
    let company = company_id(&company)?;
    let key = state.storage.recovery.read(&actor, &company).await?;
    Ok(Json(RecoveryKeyItem {
        key_id: key.key_id.to_string(),
        public: base64url::encode(&key.public),
    }))
}

pub(crate) async fn mine(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<OwnWrap>, ApiError> {
    let company = company_id(&company)?;
    let (key_id, wrap) = state.storage.recovery.mine(&actor, &company).await?;
    Ok(Json(OwnWrap {
        key_id: key_id.to_string(),
        wrap: base64url::encode(&wrap),
    }))
}
