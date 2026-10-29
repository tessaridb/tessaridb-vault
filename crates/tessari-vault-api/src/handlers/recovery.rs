//! The company recovery key (C7): owners keep it, wrapped to owners only; every member reads its public half, and an
//! owner their own wrap of it.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_constants::MAX_WRAPPED_KEYS_BYTES;
use tessari_vault_storage::group_keys::NewGroupKey;
use tessari_vault_storage::recovery::Notice;
use tessari_vault_types::base64url;

use super::group_keys::{GroupKeyBody, Pending, PendingKey, WrapsBody, decoded, key_id, wraps};
use super::{account_id, company_id};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct RecoveryKeyItem {
    key_id: String,
    public: String,
    rotation_due: bool,
}

#[derive(Serialize)]
pub(crate) struct OwnWrap {
    key_id: String,
    wrap: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Escrow {
    recovery_key_id: String,
    wrap: String,
}

#[derive(Serialize)]
pub(crate) struct NoticeItem {
    by: String,
    kind: String,
    at: i64,
}

impl From<Notice> for NoticeItem {
    fn from(notice: Notice) -> Self {
        Self {
            by: notice.by.to_string(),
            kind: notice.kind,
            at: notice.at,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Notices {
    notices: Vec<NoticeItem>,
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
        rotation_due: key.rotation_due,
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

pub(crate) async fn pending(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Pending>, ApiError> {
    let company = company_id(&company)?;
    let keys = state.storage.recovery.pending(&actor, &company).await?;
    Ok(Json(Pending {
        pending: keys.iter().map(PendingKey::from).collect(),
    }))
}

pub(crate) async fn add_wraps(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<WrapsBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    state
        .storage
        .recovery
        .add_wraps(&actor, &company, wraps(body.wraps)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn escrow(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<Escrow>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let wrap = decoded(&body.wrap)?;
    if wrap.is_empty() || wrap.len() > MAX_WRAPPED_KEYS_BYTES {
        return Err(ApiError::INVALID);
    }
    state
        .storage
        .recovery
        .escrow(&actor, &company, &key_id(&body.recovery_key_id)?, &wrap)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub(crate) struct OwnEscrow {
    recovery_key_id: String,
}

pub(crate) async fn own_escrow(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<OwnEscrow>, ApiError> {
    let company = company_id(&company)?;
    let key_id = state.storage.recovery.escrowed_to(&actor, &company).await?;
    Ok(Json(OwnEscrow {
        recovery_key_id: key_id.to_string(),
    }))
}

pub(crate) async fn escrow_of(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, account)): Path<(String, String)>,
) -> Result<Json<Escrow>, ApiError> {
    let company = company_id(&company)?;
    let account = account_id(&account)?;
    let (key_id, wrap) = state
        .storage
        .recovery
        .escrow_of(&actor, &company, &account)
        .await?;
    Ok(Json(Escrow {
        recovery_key_id: key_id.to_string(),
        wrap: base64url::encode(&wrap),
    }))
}

pub(crate) async fn notices(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Notices>, ApiError> {
    let company = company_id(&company)?;
    let notices = state.storage.recovery.notices(&actor, &company).await?;
    Ok(Json(Notices {
        notices: notices.into_iter().map(NoticeItem::from).collect(),
    }))
}
