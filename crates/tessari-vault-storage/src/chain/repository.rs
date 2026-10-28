//! What the chain service needs from the store.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Place, Value};
use tessari_vault_types::{AccountId, CompanyId};

use super::entity::{ChainEntity, Statement, StatementEntity};
use crate::Result;

/// One statement about to be appended, and who appends it when.
pub(crate) struct Append<'a> {
    pub(crate) statement: &'a Statement,
    /// The owner key the genesis pins; `None` for every later statement.
    pub(crate) owner_signing: Option<&'a [u8]>,
    pub(crate) author: &'a AccountId,
    pub(crate) now: i64,
}

#[async_trait]
pub(crate) trait ChainRepository: Send + Sync {
    /// The company's chain row and its statements, read in one script.
    async fn read(
        &self,
        place: &Place,
        company: &CompanyId,
    ) -> Result<(Option<ChainEntity>, Vec<StatementEntity>)>;
    /// Append `statement`, refused as stale unless it is the next one; the genesis also pins `owner_signing`.
    async fn append(
        &self,
        place: &Place,
        guard: Vec<(String, Value)>,
        company: &CompanyId,
        append: Append<'_>,
    ) -> Result<()>;
}
