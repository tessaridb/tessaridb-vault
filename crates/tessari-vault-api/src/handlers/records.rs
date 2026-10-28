//! A company's records (C5). The server bounds what it is sent and keeps it sealed; it reads none of it.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::records::{OpenedRecord, Record, Sealed};
use tessari_vault_types::{KeyId, RecordId, RecordType, base64url};

use super::company_id;
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct RecordItem {
    id: String,
    record_type: String,
    revision: i64,
    key_id: String,
    author: String,
    created: i64,
    updated: i64,
}

impl From<&Record> for RecordItem {
    fn from(record: &Record) -> Self {
        Self {
            id: record.id.to_string(),
            record_type: record.record_type.to_string(),
            revision: record.revision,
            key_id: record.key_id.to_string(),
            author: record.author.to_string(),
            created: record.created,
            updated: record.updated,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Records {
    records: Vec<RecordItem>,
}

#[derive(Serialize)]
pub(crate) struct Opened {
    record: RecordItem,
    body: String,
    signature: String,
    author_key: String,
    wrap: String,
}

impl From<&OpenedRecord> for Opened {
    fn from(opened: &OpenedRecord) -> Self {
        let sealed = &opened.sealed;
        Self {
            record: RecordItem::from(&opened.record),
            body: base64url::encode(sealed.body()),
            signature: base64url::encode(sealed.signature()),
            author_key: sealed.author_key().to_string(),
            wrap: base64url::encode(sealed.wrap()),
        }
    }
}

/// The sealed parts every write carries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewRecord {
    record_type: String,
    key_id: String,
    body: String,
    signature: String,
    author_key: String,
    wrap: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewRevision {
    revision: i64,
    key_id: String,
    body: String,
    signature: String,
    author_key: String,
    wrap: String,
}

fn record_id(text: &str) -> Result<RecordId, ApiError> {
    RecordId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}

fn decoded(text: &str) -> Result<Vec<u8>, ApiError> {
    base64url::decode(text).map_err(|_| ApiError::INVALID)
}

/// The record key's id and the sealed parts, each checked against its shape and ceiling.
fn sealed(
    key_id: &str,
    body: &str,
    signature: &str,
    author_key: &str,
    wrap: &str,
) -> Result<(KeyId, Sealed), ApiError> {
    let key_id = KeyId::parse(key_id).map_err(|_| ApiError::INVALID)?;
    let author_key = KeyId::parse(author_key).map_err(|_| ApiError::INVALID)?;
    let parts = Sealed::new(
        decoded(body)?,
        decoded(signature)?,
        author_key,
        decoded(wrap)?,
    )
    .map_err(|_| ApiError::INVALID)?;
    Ok((key_id, parts))
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Records>, ApiError> {
    let company = company_id(&company)?;
    let records = state.storage.records.list(&actor, &company).await?;
    Ok(Json(Records {
        records: records.iter().map(RecordItem::from).collect(),
    }))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<NewRecord>,
) -> Result<(StatusCode, Json<RecordItem>), ApiError> {
    let company = company_id(&company)?;
    let record_type = RecordType::parse(&body.record_type).map_err(|_| ApiError::INVALID)?;
    let (key_id, parts) = sealed(
        &body.key_id,
        &body.body,
        &body.signature,
        &body.author_key,
        &body.wrap,
    )?;
    let record = state
        .storage
        .records
        .create(&actor, &company, &record_type, &key_id, &parts)
        .await?;
    Ok((StatusCode::CREATED, Json(RecordItem::from(&record))))
}

pub(crate) async fn read(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record)): Path<(String, String)>,
) -> Result<Json<Opened>, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    let opened = state
        .storage
        .records
        .open(&actor, &company, &record)
        .await?;
    Ok(Json(Opened::from(&opened)))
}

pub(crate) async fn revise(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record)): Path<(String, String)>,
    Body(body): Body<NewRevision>,
) -> Result<Json<RecordItem>, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    let (key_id, parts) = sealed(
        &body.key_id,
        &body.body,
        &body.signature,
        &body.author_key,
        &body.wrap,
    )?;
    let revised = state
        .storage
        .records
        .revise(&actor, &company, &record, body.revision, &key_id, &parts)
        .await?;
    Ok(Json(RecordItem::from(&revised)))
}
