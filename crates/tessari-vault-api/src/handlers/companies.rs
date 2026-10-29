//! The caller's own account, and companies (R1).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::accounts::Account;
use tessari_vault_types::{CompanyState, Email, Name};

use super::{AccountView, company_id};
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct Me {
    account: AccountView,
    platform_admin: bool,
}

pub(crate) async fn me(
    State(state): State<AppState>,
    Caller(actor): Caller,
) -> Result<Json<Me>, ApiError> {
    let account: Account = state.storage.accounts.me(&actor).await?;
    Ok(Json(Me {
        platform_admin: account.platform_admin,
        account: AccountView::from(&account),
    }))
}

#[derive(Serialize)]
pub(crate) struct CompanyItem {
    id: String,
    name: String,
    state: &'static str,
    role: Option<&'static str>,
}

#[derive(Serialize)]
pub(crate) struct Created {
    #[serde(flatten)]
    company: CompanyItem,
    owner_account_id: String,
    /// The owner's enrolment link token, answered once; `None` when they already have a credential.
    owner_enrolment: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct Companies {
    companies: Vec<CompanyItem>,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
) -> Result<Json<Companies>, ApiError> {
    let views = state.storage.companies.list(&actor).await?;
    Ok(Json(Companies {
        companies: views
            .into_iter()
            .map(|view| CompanyItem {
                id: view.company.id.to_string(),
                name: view.company.name.to_string(),
                state: view.company.state.word(),
                role: view.role.map(|role| role.word()),
            })
            .collect(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewCompany {
    name: String,
    owner_email: String,
    owner_name: String,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Body(body): Body<NewCompany>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    let name = Name::parse(&body.name).map_err(|_| ApiError::INVALID)?;
    let email = Email::parse(&body.owner_email).map_err(|_| ApiError::INVALID)?;
    let owner = Name::parse(&body.owner_name).map_err(|_| ApiError::INVALID)?;
    let (company, owner, link) = state
        .storage
        .companies
        .create(&actor, &name, &email, &owner)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(Created {
            company: CompanyItem {
                id: company.id.to_string(),
                name: company.name.to_string(),
                state: company.state.word(),
                role: None,
            },
            owner_account_id: owner.to_string(),
            owner_enrolment: link.map(|token| token.expose().to_owned()),
        }),
    ))
}

pub(crate) async fn suspend(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<StatusCode, ApiError> {
    set_state(&state, &actor, &company, CompanyState::Suspended).await
}

pub(crate) async fn resume(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<StatusCode, ApiError> {
    set_state(&state, &actor, &company, CompanyState::Active).await
}

async fn set_state(
    state: &AppState,
    actor: &tessari_vault_storage::access::Actor,
    company: &str,
    to: CompanyState,
) -> Result<StatusCode, ApiError> {
    let company = company_id(company)?;
    state
        .storage
        .companies
        .set_state(actor, &company, to)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
