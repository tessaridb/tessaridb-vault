//! Recovery requests (C7): a member asks with a one-time handoff key; an owner hands the member's keys over to it;
//! the member redeems the handoff once and wraps their keys under a new unlock key.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_constants::{MAX_WRAPPED_KEYS_BYTES, PUBLIC_KEY_BYTES};
use tessari_vault_storage::keys::Kdf;
use tessari_vault_storage::recovery_requests::OpenRequest;
use tessari_vault_types::base64url;

use super::company_id;
use super::group_keys::{decoded, key_id};
use super::keys::KdfBody;
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AskBody {
    handoff_public: String,
}

#[derive(Serialize)]
pub(crate) struct Asked {
    id: String,
}

#[derive(Serialize)]
pub(crate) struct RequestItem {
    id: String,
    account: String,
    handoff_public: String,
    created: i64,
}

impl From<OpenRequest> for RequestItem {
    fn from(request: OpenRequest) -> Self {
        Self {
            id: request.id.to_string(),
            account: request.account.to_string(),
            handoff_public: base64url::encode(&request.handoff_public),
            created: request.created,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Requests {
    requests: Vec<RequestItem>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Handoff {
    wrap: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewrapBody {
    request: String,
    kdf: KdfBody,
    wrapped: String,
}

/// Bytes decoded from `text`, refused when empty or longer than `ceiling`.
fn bounded(text: &str, ceiling: usize) -> Result<Vec<u8>, ApiError> {
    let bytes = decoded(text)?;
    if bytes.is_empty() || bytes.len() > ceiling {
        return Err(ApiError::INVALID);
    }
    Ok(bytes)
}

pub(crate) async fn ask(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<AskBody>,
) -> Result<(StatusCode, Json<Asked>), ApiError> {
    let company = company_id(&company)?;
    let handoff = decoded(&body.handoff_public)?;
    if handoff.len() != PUBLIC_KEY_BYTES {
        return Err(ApiError::INVALID);
    }
    let id = state
        .storage
        .recovery_requests
        .ask(&actor, &company, &handoff)
        .await?;
    Ok((StatusCode::CREATED, Json(Asked { id: id.to_string() })))
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Requests>, ApiError> {
    let company = company_id(&company)?;
    let requests = state
        .storage
        .recovery_requests
        .open_requests(&actor, &company)
        .await?;
    Ok(Json(Requests {
        requests: requests.into_iter().map(RequestItem::from).collect(),
    }))
}

pub(crate) async fn hand_over(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, request)): Path<(String, String)>,
    Body(body): Body<Handoff>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let wrap = bounded(&body.wrap, MAX_WRAPPED_KEYS_BYTES)?;
    state
        .storage
        .recovery_requests
        .hand_over(&actor, &company, &key_id(&request)?, &wrap)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn redeem(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, request)): Path<(String, String)>,
) -> Result<Json<Handoff>, ApiError> {
    let company = company_id(&company)?;
    let wrap = state
        .storage
        .recovery_requests
        .redeem(&actor, &company, &key_id(&request)?)
        .await?;
    Ok(Json(Handoff {
        wrap: base64url::encode(&wrap),
    }))
}

pub(crate) async fn rewrap(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<RewrapBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let kdf = Kdf::new(
        decoded(&body.kdf.salt)?,
        body.kdf.memory_kib,
        body.kdf.passes,
        body.kdf.lanes,
    )
    .map_err(|_| ApiError::INVALID)?;
    let wrapped = bounded(&body.wrapped, MAX_WRAPPED_KEYS_BYTES)?;
    state
        .storage
        .recovery_requests
        .rewrap(&actor, &company, &key_id(&body.request)?, &kdf, &wrapped)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
