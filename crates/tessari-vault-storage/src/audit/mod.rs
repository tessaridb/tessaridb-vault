//! The company audit trail: every change an admin or a platform operator makes is recorded in the transaction that
//! makes it, so the change and its record commit together or not at all, and the response cannot leave first.
//!
//! An event records who, what, to whom, when and why as identifiers and the stated reason, never a secret. Events of
//! one company form a chain: each commits to the digest of the one before it through the company's head record, so a
//! row changed or removed behind the server's back breaks the chain at that row. Two changes to one company at once
//! contend on its head and one is refused rather than recorded out of order.

mod record;

use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tessari_vault_core::access::Action;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::CompanyId;

use crate::access::{self, Actor, bind};
use crate::sessions::hex;
use crate::{Error, Result};
pub use record::AuditEntry;
use record::Chained;

/// The statements that record the event bound by [`binds`]; spliced into a script before its `COMMIT`.
pub(crate) const RECORD: &str = "\
LET $audit_prev = (GET audit_heads:$audit_company).digest ?? ''; \
LET $audit_seq = ((GET audit_heads:$audit_company).seq ?? 0) + 1; \
LET $audit_digest = crypto::sha256(array::join([$audit_prev, $audit_line], '|')); \
CREATE audit_events = { company: $audit_company, seq: $audit_seq, at: $audit_at, actor: $audit_actor, \
action: $audit_action, target: $audit_target, reason: $audit_reason, line: $audit_line, prev: $audit_prev, \
digest: $audit_digest }; \
UPSERT audit_heads:$audit_company = { digest: $audit_digest, seq: $audit_seq };";

/// The most events one page of the trail answers.
const PAGE: i64 = 100;

/// What an event is about.
pub(crate) struct Event<'a> {
    /// The company whose trail it joins.
    pub(crate) company: &'a str,
    /// What was done, as a stable dotted name.
    pub(crate) action: &'static str,
    /// To whom or what: an account or group id.
    pub(crate) target: &'a str,
}

/// The canonical text an event's digest covers: every part length-prefixed, so no part can move another's boundary.
fn line(at: i64, company: &str, actor: &str, action: &str, target: &str, reason: &str) -> String {
    let mut text = String::from("v1");
    for part in [
        at.to_string().as_str(),
        company,
        actor,
        action,
        target,
        reason,
    ] {
        text.push_str(&format!("|{}:{part}", part.len()));
    }
    text
}

/// The values [`RECORD`] binds, for `event` done by `actor` now.
pub(crate) fn binds(actor: &Actor, event: &Event<'_>) -> Vec<(String, Value)> {
    let at = epoch_seconds();
    let text = line(
        at,
        event.company,
        actor.account().as_str(),
        event.action,
        event.target,
        actor.reason(),
    );
    vec![
        bind("audit_company", event.company),
        bind("audit_at", at),
        bind("audit_actor", actor.account().as_str()),
        bind("audit_action", event.action),
        bind("audit_target", event.target),
        bind("audit_reason", actor.reason()),
        bind("audit_line", text.as_str()),
    ]
}

/// Now, in seconds since 1970.
pub(crate) fn epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// The audit trail, read.
pub struct AuditService {
    database: Database,
    place: Place,
}

impl AuditService {
    pub(crate) const fn new(database: Database, place: Place) -> Self {
        Self { database, place }
    }

    /// The company's events, newest first: a page of at most 100, those before `before` when it is given.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an admin or owner of the open company; otherwise when the store fails.
    pub async fn list(
        &self,
        actor: &Actor,
        company: &CompanyId,
        before: Option<i64>,
    ) -> Result<Vec<AuditEntry>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadAudit,
        )
        .await?;
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT seq, at, actor, action, target, reason FROM audit_events \
                 WHERE company = $company AND seq < $before ORDER BY seq DESC LIMIT $page;",
                vec![
                    bind("company", company.as_str()),
                    bind("before", before.unwrap_or(i64::MAX)),
                    bind("page", PAGE),
                ],
            )
            .await?;
        crate::answers::records(answers, "audit event")
    }

    /// Walk the company's whole chain: the sequence number of the first event whose columns, line or digest do not
    /// follow from the one before it; `None` when the chain holds and the head ends on its last event.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] unless the caller is an admin or owner of the open company; otherwise when the store fails.
    pub async fn verify(&self, actor: &Actor, company: &CompanyId) -> Result<Option<i64>> {
        access::authorize(
            &self.database,
            &self.place,
            actor,
            company,
            Action::ReadAudit,
        )
        .await?;
        let answers = self
            .database
            .read_at(
                &self.place,
                "SELECT seq, at, company, actor, action, target, reason, line, prev, digest FROM audit_events \
                 WHERE company = $company ORDER BY seq;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        let chain: Vec<Chained> = crate::answers::records(answers, "audit event")?;
        let mut previous = String::new();
        let mut expected_seq = 1_i64;
        for event in &chain {
            let shown = &event.shown;
            // The line is rebuilt from the columns, so a column changed behind the server's back breaks here too.
            let rebuilt = line(
                shown.at,
                &event.company,
                &shown.actor,
                &shown.action,
                &shown.target,
                &shown.reason,
            );
            let digest = hex(&Sha256::digest(
                format!("{}|{}", event.prev, rebuilt).as_bytes(),
            ));
            if shown.seq != expected_seq
                || rebuilt != event.line
                || event.prev != previous
                || event.digest != digest
            {
                return Ok(Some(shown.seq));
            }
            previous = digest;
            expected_seq = expected_seq.saturating_add(1);
        }
        let head = self
            .database
            .read_at(
                &self.place,
                "GET audit_heads:$company;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        let held =
            crate::enrolments::record::field(head.into_iter().next(), "digest").unwrap_or_default();
        if held == previous {
            Ok(None)
        } else {
            Err(Error::Unreadable {
                record: "audit head",
                reason: "the head does not end on the last event".to_owned(),
            })
        }
    }
}
