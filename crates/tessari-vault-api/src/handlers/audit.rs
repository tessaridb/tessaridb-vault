//! The company audit trail, for its admins and owners: the events, newest first, and whether the chain holds.

use axum::Json;
use axum::extract::{Path, RawQuery, State};
use serde::Serialize;
use tessari_vault_storage::audit::AuditEntry;

use crate::ApiError;
use crate::caller::Caller;
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct Event {
    seq: i64,
    at: i64,
    actor: String,
    action: String,
    target: String,
    reason: String,
}

impl From<AuditEntry> for Event {
    fn from(entry: AuditEntry) -> Self {
        Self {
            seq: entry.seq,
            at: entry.at,
            actor: entry.actor,
            action: entry.action,
            target: entry.target,
            reason: entry.reason,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Events {
    events: Vec<Event>,
}

#[derive(Serialize)]
pub(crate) struct Verified {
    intact: bool,
    broken_at: Option<i64>,
}

/// The `before` a page query names: a sequence number, or none for the newest page.
fn before(query: Option<&str>) -> Result<Option<i64>, ApiError> {
    let Some(query) = query else {
        return Ok(None);
    };
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == "before")
        .map(|(_, value)| value.parse().map_err(|_| ApiError::INVALID))
        .transpose()
}

/// The company's events, newest first, a page at a time.
pub(crate) async fn list(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Json<Events>, ApiError> {
    let company = super::company_id(&company)?;
    let events = state
        .storage
        .audit
        .list(&actor, &company, before(query.as_deref())?)
        .await?;
    Ok(Json(Events {
        events: events.into_iter().map(Event::from).collect(),
    }))
}

/// Walk the whole chain: whether it holds, and where it first breaks.
pub(crate) async fn verify(
    State(state): State<AppState>,
    Caller(actor): Caller,
    Path(company): Path<String>,
) -> Result<Json<Verified>, ApiError> {
    let company = super::company_id(&company)?;
    let broken_at = state.storage.audit.verify(&actor, &company).await?;
    Ok(Json(Verified {
        intact: broken_at.is_none(),
        broken_at,
    }))
}
