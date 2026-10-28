//! Encrypted files on a record (C5): started with their sealed size, written in parts in order, read in ranges.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tessari_vault_constants::{FILE_PART_MAX_BYTES, MAX_FILE_BYTES};
use tessari_vault_storage::files::FileMeta;
use tessari_vault_types::{FileId, RecordId};

use super::company_id;
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct FileItem {
    id: String,
    size: u64,
    kept: &'static str,
    written: u64,
}

impl From<&FileMeta> for FileItem {
    fn from(meta: &FileMeta) -> Self {
        Self {
            id: meta.id.to_string(),
            size: meta.size,
            kept: meta.kept.word(),
            written: meta.written,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewFile {
    size: u64,
}

/// The whole numbers `names` from a query string holding exactly those keys, each once.
fn numbers<const N: usize>(query: Option<&str>, names: [&str; N]) -> Result<[u64; N], ApiError> {
    let mut found = [None; N];
    for (key, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        let slot = names
            .iter()
            .position(|name| *name == key)
            .and_then(|at| found.get_mut(at))
            .filter(|slot| slot.is_none())
            .ok_or(ApiError::INVALID)?;
        *slot = Some(value.parse::<u64>().map_err(|_| ApiError::INVALID)?);
    }
    let mut numbers = [0; N];
    for (number, value) in numbers.iter_mut().zip(found) {
        *number = value.ok_or(ApiError::INVALID)?;
    }
    Ok(numbers)
}

// Ids in a path that do not parse name nothing, so they are answered as missing things are.

fn record_id(text: &str) -> Result<RecordId, ApiError> {
    RecordId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}

fn file_id(text: &str) -> Result<FileId, ApiError> {
    FileId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record)): Path<(String, String)>,
    Body(body): Body<NewFile>,
) -> Result<(StatusCode, Json<FileItem>), ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    if body.size == 0 || body.size > MAX_FILE_BYTES {
        return Err(ApiError::INVALID);
    }
    let meta = state
        .storage
        .files
        .create(&actor, &company, &record, body.size)
        .await?;
    Ok((StatusCode::CREATED, Json(FileItem::from(&meta))))
}

pub(crate) async fn write(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record, file)): Path<(String, String, String)>,
    RawQuery(query): RawQuery,
    bytes: Bytes,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    let file = file_id(&file)?;
    let [offset] = numbers(query.as_deref(), ["offset"])?;
    state
        .storage
        .files
        .write(&actor, &company, &record, &file, offset, &bytes)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn read(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path((company, record, file)): Path<(String, String, String)>,
    RawQuery(query): RawQuery,
) -> Result<impl IntoResponse, ApiError> {
    let company = company_id(&company)?;
    let record = record_id(&record)?;
    let file = file_id(&file)?;
    let [offset, limit] = numbers(query.as_deref(), ["offset", "limit"])?;
    if limit == 0 || limit > FILE_PART_MAX_BYTES {
        return Err(ApiError::INVALID);
    }
    let bytes = state
        .storage
        .files
        .read(&actor, &company, &record, &file, offset, limit)
        .await?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes))
}
