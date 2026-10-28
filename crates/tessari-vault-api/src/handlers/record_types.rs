//! A company's own record types (C5): who may create records of each, and the record holding its definition.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::record_types::RecordTypeDefinition;
use tessari_vault_types::{RecordId, Roles};

use super::company_id;
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct RecordTypeItem {
    id: String,
    allowed_roles: Vec<&'static str>,
    definition: String,
    created: i64,
}

impl From<&RecordTypeDefinition> for RecordTypeItem {
    fn from(definition: &RecordTypeDefinition) -> Self {
        Self {
            id: definition.id.to_string(),
            allowed_roles: definition.allowed.words(),
            definition: definition.definition.to_string(),
            created: definition.created,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct RecordTypes {
    record_types: Vec<RecordTypeItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewRecordType {
    allowed_roles: Vec<String>,
    definition: String,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<RecordTypes>, ApiError> {
    let company = company_id(&company)?;
    let types = state.storage.record_types.list(&actor, &company).await?;
    Ok(Json(RecordTypes {
        record_types: types.iter().map(RecordTypeItem::from).collect(),
    }))
}

pub(crate) async fn define(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<NewRecordType>,
) -> Result<(StatusCode, Json<RecordTypeItem>), ApiError> {
    let company = company_id(&company)?;
    let allowed = Roles::parse(body.allowed_roles.iter().map(String::as_str))
        .map_err(|_| ApiError::INVALID)?;
    // A definition id that does not parse names no record, so it is answered as a missing one is.
    let definition = RecordId::parse(&body.definition).map_err(|_| ApiError::NOT_FOUND)?;
    let defined = state
        .storage
        .record_types
        .define(&actor, &company, allowed, &definition)
        .await?;
    Ok((StatusCode::CREATED, Json(RecordTypeItem::from(&defined))))
}
