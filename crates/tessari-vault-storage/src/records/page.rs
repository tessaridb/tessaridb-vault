//! A page of opened records: the records the caller reaches, in id order after a cursor, each opened by the same path
//! a single read takes, so every bundle in the page is its own read on the company's trail.

use tessari_vault_types::{CompanyId, RecordId};

use super::entity::OpenedRecord;
use super::service::RecordService;
use crate::Result;
use crate::access::Actor;

/// Opened records and the cursor to the next page, absent on the last one.
pub struct OpenedPage {
    /// The records of this page, opened.
    pub records: Vec<OpenedRecord>,
    /// The id the next page starts after.
    pub next: Option<RecordId>,
}

impl RecordService {
    /// Up to `limit` records the caller reaches whose id follows `after`, each opened and recorded as it is read.
    ///
    /// # Errors
    ///
    /// As [`RecordService::list`] and [`RecordService::open`].
    pub async fn open_page(
        &self,
        actor: &Actor,
        company: &CompanyId,
        after: Option<&RecordId>,
        limit: usize,
    ) -> Result<OpenedPage> {
        let mut ids: Vec<RecordId> = self
            .list(actor, company)
            .await?
            .into_iter()
            .map(|record| record.id)
            .filter(|id| after.is_none_or(|after| id > after))
            .collect();
        ids.sort_unstable();
        let more = ids.len() > limit;
        ids.truncate(limit);
        let mut records = Vec::with_capacity(ids.len());
        for id in &ids {
            records.push(self.open(actor, company, id).await?);
        }
        let next = if more { ids.last().cloned() } else { None };
        Ok(OpenedPage { records, next })
    }
}
