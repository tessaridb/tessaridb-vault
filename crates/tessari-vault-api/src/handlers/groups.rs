//! A company's departments and teams (R3).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::groups::Group;
use tessari_vault_types::{GroupKind, Name};

use super::{AccountView, account_id, company_id, group_id};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct GroupItem {
    id: String,
    kind: &'static str,
    name: String,
    parent: String,
}

impl From<&Group> for GroupItem {
    fn from(group: &Group) -> Self {
        Self {
            id: group.id.to_string(),
            kind: group.kind.word(),
            name: group.name.to_string(),
            parent: group.parent.to_string(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Groups {
    groups: Vec<GroupItem>,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Groups>, ApiError> {
    let company = company_id(&company)?;
    let groups = state.storage.groups.list(&actor, &company).await?;
    Ok(Json(Groups {
        groups: groups.iter().map(GroupItem::from).collect(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewGroup {
    kind: String,
    name: String,
    parent: Option<String>,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<NewGroup>,
) -> Result<(StatusCode, Json<GroupItem>), ApiError> {
    let company = company_id(&company)?;
    let kind: GroupKind = body.kind.parse().map_err(|_| ApiError::INVALID)?;
    let name = Name::parse(&body.name).map_err(|_| ApiError::INVALID)?;
    let parent = body.parent.as_deref().map(group_id).transpose()?;
    let group = state
        .storage
        .groups
        .create(&actor, &company, kind, &name, parent.as_ref())
        .await?;
    Ok((StatusCode::CREATED, Json(GroupItem::from(&group))))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rename {
    name: String,
}

pub(crate) async fn rename(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
    Body(body): Body<Rename>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    let name = Name::parse(&body.name).map_err(|_| ApiError::INVALID)?;
    state
        .storage
        .groups
        .rename(&actor, &company, &group, &name)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    state
        .storage
        .groups
        .delete(&actor, &company, &group)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub(crate) struct GroupMembers {
    members: Vec<AccountView>,
}

pub(crate) async fn members(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
) -> Result<Json<GroupMembers>, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    let accounts = state
        .storage
        .groups
        .members(&actor, &company, &group)
        .await?;
    Ok(Json(GroupMembers {
        members: accounts.iter().map(AccountView::from).collect(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewGroupMember {
    account_id: String,
}

pub(crate) async fn add_member(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group)): Path<(String, String)>,
    Body(body): Body<NewGroupMember>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    // An id that does not parse is refused as malformed, the same answer an unknown member's id would not get:
    // that one is a conflict, decided inside the company the caller may manage.
    let account = account_id(&body.account_id).map_err(|_| ApiError::INVALID)?;
    state
        .storage
        .groups
        .add_member(&actor, &company, &group, &account)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn remove_member(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, group, account)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let group = group_id(&group)?;
    let account = account_id(&account)?;
    state
        .storage
        .groups
        .remove_member(&actor, &company, &group, &account)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
