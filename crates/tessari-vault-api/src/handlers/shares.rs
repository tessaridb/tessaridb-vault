//! Sharing a record with a group or a member (C4). The browser wraps the record key to the target's key; the server
//! keeps the wrap and the share together and reads neither the key nor the record.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use tessari_vault_storage::shares::{Access, NewShare, ShareTarget};
use tessari_vault_types::KeyId;

use super::records::{decoded, record_id};
use super::{account_id, company_id, group_id};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

/// A share: exactly one of `group` and `member`, the access it gives, and the record key wrapped to its key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShareBody {
    group: Option<String>,
    member: Option<String>,
    right: String,
    key_id: String,
    wrap: String,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record)): Path<(String, String)>,
    Body(body): Body<ShareBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    let target = match (&body.group, &body.member) {
        (Some(group), None) => ShareTarget::Group(group_id(group)?),
        (None, Some(member)) => ShareTarget::Member(account_id(member)?),
        _ => return Err(ApiError::INVALID),
    };
    let access = Access::parse(&body.right).ok_or(ApiError::INVALID)?;
    let key_id = KeyId::parse(&body.key_id).map_err(|_| ApiError::INVALID)?;
    let share = NewShare::new(target, access, key_id, decoded(&body.wrap)?)
        .map_err(|_| ApiError::INVALID)?;
    state
        .storage
        .shares
        .share(&actor, &company, &record, &share)
        .await?;
    Ok(StatusCode::CREATED)
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record, target)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    state
        .storage
        .shares
        .remove(&actor, &company, &record, &target)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
