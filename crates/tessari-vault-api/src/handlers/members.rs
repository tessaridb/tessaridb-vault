//! A company's members (R2).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_types::{Email, MemberState, Name, Role};

use super::{AccountView, account_id, company_id};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct MemberItem {
    account: AccountView,
    role: &'static str,
    state: &'static str,
}

#[derive(Serialize)]
pub(crate) struct Members {
    members: Vec<MemberItem>,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Members>, ApiError> {
    let company = company_id(&company)?;
    let members = state.storage.companies.members(&actor, &company).await?;
    Ok(Json(Members {
        members: members
            .iter()
            .map(|member| MemberItem {
                account: AccountView::from(&member.account),
                role: member.role.word(),
                state: member.state.word(),
            })
            .collect(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewMember {
    email: String,
    name: String,
    role: String,
}

#[derive(Serialize)]
pub(crate) struct Added {
    account_id: String,
}

pub(crate) async fn add(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<NewMember>,
) -> Result<(StatusCode, Json<Added>), ApiError> {
    let company = company_id(&company)?;
    let email = Email::parse(&body.email).map_err(|_| ApiError::INVALID)?;
    let name = Name::parse(&body.name).map_err(|_| ApiError::INVALID)?;
    let role: Role = body.role.parse().map_err(|_| ApiError::INVALID)?;
    let account = state
        .storage
        .companies
        .add_member(&actor, &company, &email, &name, role)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(Added {
            account_id: account.to_string(),
        }),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberChange {
    role: Option<String>,
    state: Option<String>,
}

pub(crate) async fn change(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, account)): Path<(String, String)>,
    Body(body): Body<MemberChange>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let account = account_id(&account)?;
    let role = body
        .role
        .as_deref()
        .map(str::parse::<Role>)
        .transpose()
        .map_err(|_| ApiError::INVALID)?;
    let member_state = body
        .state
        .as_deref()
        .map(str::parse::<MemberState>)
        .transpose()
        .map_err(|_| ApiError::INVALID)?;
    if role.is_none() && member_state.is_none() {
        return Err(ApiError::INVALID);
    }
    state
        .storage
        .companies
        .change_member(&actor, &company, &account, role, member_state)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, account)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let account = account_id(&account)?;
    state
        .storage
        .companies
        .remove_member(&actor, &company, &account)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
