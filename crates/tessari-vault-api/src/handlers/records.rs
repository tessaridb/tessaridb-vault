//! A company's records (C5). The server bounds what it is sent and keeps it sealed; it reads none of it.

use axum::Json;
use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tessari_vault_constants::{MAX_RECORD_WRAP_BYTES, RECORD_PAGE_MAX};
use tessari_vault_storage::records::{OpenedRecord, Record, Revision, Sealed};
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
    rotation_due: bool,
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
            rotation_due: record.rotation_due,
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
    /// The position of the company's key chain the latest revision was signed at.
    chain_seq: i64,
    recipient: String,
    wrap: String,
    through: Vec<String>,
    revised_by: String,
}

impl From<&OpenedRecord> for Opened {
    fn from(opened: &OpenedRecord) -> Self {
        let sealed = &opened.sealed;
        Self {
            record: RecordItem::from(&opened.record),
            body: base64url::encode(sealed.body()),
            signature: base64url::encode(sealed.signature()),
            author_key: sealed.author_key().to_string(),
            chain_seq: sealed.chain_seq(),
            recipient: opened.recipient.clone(),
            wrap: base64url::encode(sealed.wrap()),
            through: opened.through.clone(),
            revised_by: opened.revised_by.clone(),
        }
    }
}

/// The sealed parts every write carries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewRecord {
    /// The id the browser sealed revision 1 under.
    id: String,
    record_type: String,
    key_id: String,
    body: String,
    signature: String,
    author_key: String,
    /// The position of the company's key chain the revision was signed at.
    chain_seq: i64,
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
    /// The position of the company's key chain the revision was signed at.
    chain_seq: i64,
    wrap: String,
    /// The record key wrapped to every other recipient the record keeps.
    #[serde(default)]
    wraps: Vec<RecipientWrap>,
}

/// The record key wrapped to one recipient, `member:<key id>` or `group:<key id>`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecipientWrap {
    recipient: String,
    wrap: String,
}

/// A recipient name and its wrap, each within its shape and ceiling.
fn recipient_wrap(given: &RecipientWrap) -> Result<(String, Vec<u8>), ApiError> {
    let shaped = given
        .recipient
        .split_once(':')
        .is_some_and(|(kind, key)| matches!(kind, "member" | "group") && KeyId::parse(key).is_ok());
    let wrap = decoded(&given.wrap)?;
    if !shaped || wrap.is_empty() || wrap.len() > MAX_RECORD_WRAP_BYTES {
        return Err(ApiError::INVALID);
    }
    Ok((given.recipient.clone(), wrap))
}

pub(crate) fn record_id(text: &str) -> Result<RecordId, ApiError> {
    RecordId::parse(text).map_err(|_| ApiError::NOT_FOUND)
}

pub(crate) fn decoded(text: &str) -> Result<Vec<u8>, ApiError> {
    base64url::decode(text).map_err(|_| ApiError::INVALID)
}

/// The record key's id and the sealed parts, each checked against its shape and ceiling.
fn sealed(
    key_id: &str,
    body: &str,
    signature: &str,
    author_key: &str,
    chain_seq: i64,
    wrap: &str,
) -> Result<(KeyId, Sealed), ApiError> {
    let key_id = KeyId::parse(key_id).map_err(|_| ApiError::INVALID)?;
    let author_key = KeyId::parse(author_key).map_err(|_| ApiError::INVALID)?;
    let parts = Sealed::new(
        decoded(body)?,
        decoded(signature)?,
        author_key,
        chain_seq,
        decoded(wrap)?,
    )
    .map_err(|_| ApiError::INVALID)?;
    Ok((key_id, parts))
}

/// How a listing is asked for: plain, or as a page of opened records (`opened=true`, a cursor and a size).
#[derive(Default)]
struct Listing {
    opened: bool,
    after: Option<RecordId>,
    limit: Option<usize>,
}

/// The listing a query string asks for: only `opened=true`, `after` and `limit`, each at most once.
fn listing(query: Option<&str>) -> Result<Listing, ApiError> {
    let mut listing = Listing::default();
    let mut seen = [false; 3];
    for (key, value) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        let at = match key.as_ref() {
            "opened" if value == "true" => 0,
            "after" => 1,
            "limit" => 2,
            _ => return Err(ApiError::INVALID),
        };
        if seen.get(at).copied().unwrap_or(true) {
            return Err(ApiError::INVALID);
        }
        if let Some(slot) = seen.get_mut(at) {
            *slot = true;
        }
        match at {
            0 => listing.opened = true,
            1 => listing.after = Some(RecordId::parse(&value).map_err(|_| ApiError::INVALID)?),
            _ => listing.limit = Some(value.parse().map_err(|_| ApiError::INVALID)?),
        }
    }
    Ok(listing)
}

/// A page of opened records and the cursor to the next.
#[derive(Serialize)]
pub(crate) struct OpenedRecords {
    records: Vec<Opened>,
    next: Option<String>,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let company = company_id(&company)?;
    let listing = listing(query.as_deref())?;
    if !listing.opened {
        if listing.after.is_some() || listing.limit.is_some() {
            return Err(ApiError::INVALID);
        }
        let records = state.storage.records.list(&actor, &company).await?;
        return Ok(Json(Records {
            records: records.iter().map(RecordItem::from).collect(),
        })
        .into_response());
    }
    let limit = listing.limit.unwrap_or(RECORD_PAGE_MAX);
    if limit == 0 || limit > RECORD_PAGE_MAX {
        return Err(ApiError::INVALID);
    }
    let page = state
        .storage
        .records
        .open_page(&actor, &company, listing.after.as_ref(), limit)
        .await?;
    Ok(Json(OpenedRecords {
        records: page.records.iter().map(Opened::from).collect(),
        next: page.next.map(|id| id.to_string()),
    })
    .into_response())
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<NewRecord>,
) -> Result<(StatusCode, Json<RecordItem>), ApiError> {
    let company = company_id(&company)?;
    let id = RecordId::parse(&body.id).map_err(|_| ApiError::INVALID)?;
    let record_type = RecordType::parse(&body.record_type).map_err(|_| ApiError::INVALID)?;
    let (key_id, parts) = sealed(
        &body.key_id,
        &body.body,
        &body.signature,
        &body.author_key,
        body.chain_seq,
        &body.wrap,
    )?;
    let record = state
        .storage
        .records
        .create(&actor, &company, &id, &record_type, &key_id, &parts)
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
        body.chain_seq,
        &body.wrap,
    )?;
    let wraps = body
        .wraps
        .iter()
        .map(recipient_wrap)
        .collect::<Result<Vec<_>, _>>()?;
    let revised = state
        .storage
        .records
        .revise(
            &actor,
            &company,
            &record,
            Revision {
                expected: body.revision,
                key_id: &key_id,
                sealed: &parts,
                wraps: &wraps,
            },
        )
        .await?;
    Ok(Json(RecordItem::from(&revised)))
}
