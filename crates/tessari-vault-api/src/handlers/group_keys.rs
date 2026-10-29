//! Group keys (KH-L1): admins key a group and wrap its private key to its members; every member reads a group's key
//! and their own wrap of it.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::group_keys::{MemberWrap, NewGroupKey};
use tessari_vault_storage::keys::PublicKeys;
use tessari_vault_types::{KeyId, base64url};

use super::{company_id, group_id};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WrapBody {
    key_id: String,
    wrap: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupKeyBody {
    key_id: String,
    public: String,
    wraps: Vec<WrapBody>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WrapsBody {
    wraps: Vec<WrapBody>,
}

#[derive(Serialize)]
pub(crate) struct GroupKeyItem {
    key_id: String,
    public: String,
    wrap: Option<String>,
    rotation_due: bool,
}

#[derive(Serialize)]
pub(crate) struct PendingKey {
    account: String,
    key_id: String,
    agreement_public: String,
}

impl From<&PublicKeys> for PendingKey {
    fn from(keys: &PublicKeys) -> Self {
        Self {
            account: keys.account.to_string(),
            key_id: keys.key_id.to_string(),
            agreement_public: base64url::encode(&keys.agreement_public),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Pending {
    pending: Vec<PendingKey>,
}

fn decoded(text: &str) -> Result<Vec<u8>, ApiError> {
    base64url::decode(text).map_err(|_| ApiError::INVALID)
}

fn key_id(text: &str) -> Result<KeyId, ApiError> {
    KeyId::parse(text).map_err(|_| ApiError::INVALID)
}

fn wraps(bodies: Vec<WrapBody>) -> Result<Vec<MemberWrap>, ApiError> {
    bodies
        .into_iter()
        .map(|body| {
            MemberWrap::new(key_id(&body.key_id)?, decoded(&body.wrap)?)
                .map_err(|_| ApiError::INVALID)
        })
        .collect()
}

pub(crate) async fn keep(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
    Body(body): Body<GroupKeyBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    let key = NewGroupKey::new(
        key_id(&body.key_id)?,
        decoded(&body.public)?,
        wraps(body.wraps)?,
    )
    .map_err(|_| ApiError::INVALID)?;
    state
        .storage
        .group_keys
        .keep(&actor, &company, &group, key)
        .await?;
    Ok(StatusCode::CREATED)
}

pub(crate) async fn read(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
) -> Result<Json<GroupKeyItem>, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    let key = state
        .storage
        .group_keys
        .read(&actor, &company, &group)
        .await?;
    Ok(Json(GroupKeyItem {
        key_id: key.key_id.to_string(),
        public: base64url::encode(&key.public),
        wrap: key.wrap.as_deref().map(base64url::encode),
        rotation_due: key.rotation_due,
    }))
}

pub(crate) async fn pending(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
) -> Result<Json<Pending>, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    let keys = state
        .storage
        .group_keys
        .pending(&actor, &company, &group)
        .await?;
    Ok(Json(Pending {
        pending: keys.iter().map(PendingKey::from).collect(),
    }))
}

pub(crate) async fn add_wraps(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
    Body(body): Body<WrapsBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    state
        .storage
        .group_keys
        .add_wraps(&actor, &company, &group, wraps(body.wraps)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
