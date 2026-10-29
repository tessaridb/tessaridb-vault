//! Sharing a record with a group or a member (C4). The browser wraps the record key to the target's key; the server
//! keeps the wrap and the share together and reads neither the key nor the record.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::shares::{Access, ListedShares, NewShare, ShareTarget};
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

/// A record's author and its key set, as a revision wraps to them.
#[derive(Serialize)]
pub(crate) struct AuthorItem {
    account: String,
    key_id: Option<String>,
}

/// One share: `kind` is `group` or `member`, `target` its id, `right` what it lets them do, `key_id` the key a
/// revision wraps the record key to for them, under the recipient name `<kind>:<key_id>`.
#[derive(Serialize)]
pub(crate) struct ShareItem {
    kind: &'static str,
    target: String,
    right: &'static str,
    key_id: String,
}

#[derive(Serialize)]
pub(crate) struct SharesAnswer {
    author: AuthorItem,
    shares: Vec<ShareItem>,
}

impl From<ListedShares> for SharesAnswer {
    fn from(listed: ListedShares) -> Self {
        Self {
            author: AuthorItem {
                account: listed.author.as_str().to_owned(),
                key_id: listed.author_key.map(|key| key.as_str().to_owned()),
            },
            shares: listed
                .shares
                .into_iter()
                .map(|share| {
                    let (kind, target) = match share.target {
                        ShareTarget::Group(group) => ("group", group.as_str().to_owned()),
                        ShareTarget::Member(account) => ("member", account.as_str().to_owned()),
                    };
                    ShareItem {
                        kind,
                        target,
                        right: share.access.word(),
                        key_id: share.key_id.as_str().to_owned(),
                    }
                })
                .collect(),
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record)): Path<(String, String)>,
) -> Result<Json<SharesAnswer>, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    let listed = state.storage.shares.list(&actor, &company, &record).await?;
    Ok(Json(SharesAnswer::from(listed)))
}
