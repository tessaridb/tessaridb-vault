//! Member key sets and the vouching chain (KH-L1, KH-L3). The server bounds what it is sent and keeps it; the
//! browsers check what it says.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tessari_vault_storage::chain::Statement;
use tessari_vault_storage::keys::{Kdf, KeySet, NewKeys, PublicKeys};
use tessari_vault_types::{KeyId, base64url};

use super::company_id;
use crate::ApiError;
use crate::caller::{Body, Caller};
use crate::state::AppState;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KdfBody {
    salt: String,
    memory_kib: u32,
    passes: u32,
    lanes: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeySetBody {
    key_id: String,
    agreement_public: String,
    signing_public: String,
    wrapped: String,
    kdf: KdfBody,
}

#[derive(Serialize)]
pub(crate) struct OwnKeys {
    key_id: String,
    agreement_public: String,
    signing_public: String,
    wrapped: String,
    kdf: KdfBody,
}

impl From<&KeySet> for OwnKeys {
    fn from(set: &KeySet) -> Self {
        Self {
            key_id: set.public.key_id.to_string(),
            agreement_public: base64url::encode(&set.public.agreement_public),
            signing_public: base64url::encode(&set.public.signing_public),
            wrapped: base64url::encode(set.wrapped()),
            kdf: KdfBody {
                salt: base64url::encode(set.kdf.salt()),
                memory_kib: set.kdf.memory_kib(),
                passes: set.kdf.passes(),
                lanes: set.kdf.lanes(),
            },
        }
    }
}

#[derive(Serialize)]
pub(crate) struct DirectoryEntry {
    account: String,
    key_id: String,
    agreement_public: String,
    signing_public: String,
}

impl From<&PublicKeys> for DirectoryEntry {
    fn from(keys: &PublicKeys) -> Self {
        Self {
            account: keys.account.to_string(),
            key_id: keys.key_id.to_string(),
            agreement_public: base64url::encode(&keys.agreement_public),
            signing_public: base64url::encode(&keys.signing_public),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Directory {
    keys: Vec<DirectoryEntry>,
}

#[derive(Serialize)]
pub(crate) struct Kept {
    key_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatementBody {
    seq: u64,
    bytes: String,
    signature: String,
    owner_signing: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct StatementItem {
    seq: u64,
    bytes: String,
    signature: String,
}

#[derive(Serialize)]
pub(crate) struct ChainItem {
    owner_signing: Option<String>,
    statements: Vec<StatementItem>,
}

fn decoded(text: &str) -> Result<Vec<u8>, ApiError> {
    base64url::decode(text).map_err(|_| ApiError::INVALID)
}

pub(crate) async fn keep(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<KeySetBody>,
) -> Result<(StatusCode, Json<Kept>), ApiError> {
    let company = company_id(&company)?;
    let kdf = Kdf::new(
        decoded(&body.kdf.salt)?,
        body.kdf.memory_kib,
        body.kdf.passes,
        body.kdf.lanes,
    )
    .map_err(|_| ApiError::INVALID)?;
    let new = NewKeys::new(
        KeyId::parse(&body.key_id).map_err(|_| ApiError::INVALID)?,
        decoded(&body.agreement_public)?,
        decoded(&body.signing_public)?,
        kdf,
        decoded(&body.wrapped)?,
    )
    .map_err(|_| ApiError::INVALID)?;
    let kept = state.storage.keys.keep(&actor, &company, new).await?;
    Ok((
        StatusCode::CREATED,
        Json(Kept {
            key_id: kept.key_id.to_string(),
        }),
    ))
}

pub(crate) async fn mine(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<OwnKeys>, ApiError> {
    let company = company_id(&company)?;
    let set = state.storage.keys.mine(&actor, &company).await?;
    Ok(Json(OwnKeys::from(&set)))
}

pub(crate) async fn directory(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Directory>, ApiError> {
    let company = company_id(&company)?;
    let keys = state.storage.keys.directory(&actor, &company).await?;
    Ok(Json(Directory {
        keys: keys.iter().map(DirectoryEntry::from).collect(),
    }))
}

pub(crate) async fn chain(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<ChainItem>, ApiError> {
    let company = company_id(&company)?;
    let chain = state.storage.chain.read(&actor, &company).await?;
    Ok(Json(ChainItem {
        owner_signing: chain.owner_signing.as_deref().map(base64url::encode),
        statements: chain
            .statements
            .iter()
            .map(|statement| StatementItem {
                seq: statement.seq(),
                bytes: base64url::encode(statement.bytes()),
                signature: base64url::encode(statement.signature()),
            })
            .collect(),
    }))
}

pub(crate) async fn append(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    Body(body): Body<StatementBody>,
) -> Result<StatusCode, ApiError> {
    let company = company_id(&company)?;
    // The genesis and only the genesis pins the owner key the chain is checked from.
    if (body.seq == 1) != body.owner_signing.is_some() {
        return Err(ApiError::INVALID);
    }
    let statement = Statement::new(body.seq, decoded(&body.bytes)?, decoded(&body.signature)?)
        .map_err(|_| ApiError::INVALID)?;
    let owner_signing = body.owner_signing.as_deref().map(decoded).transpose()?;
    state
        .storage
        .chain
        .append(&actor, &company, statement, owner_signing)
        .await?;
    Ok(StatusCode::CREATED)
}
