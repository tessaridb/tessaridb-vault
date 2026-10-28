//! Each company's records vault: declared in key custody when the company is created, and opened only by a request
//! that presents the company's key (layer 2, LR-VA-005). The key is derived per request and never stored.

use std::sync::Arc;

use tessari_vault_infrastructure::custody::VaultKeys;
use tessari_vault_infrastructure::tessaridb::{Database, Place, VaultKey};
use tessari_vault_types::CompanyId;

use crate::Result;

/// The records vaults, and the keys that open them.
pub(crate) struct RecordVaults {
    database: Database,
    keys: Arc<VaultKeys>,
}

impl RecordVaults {
    pub(crate) const fn new(database: Database, keys: Arc<VaultKeys>) -> Self {
        Self { database, keys }
    }

    /// The vault holding `company`'s record bodies. A company id is 32 lowercase hexadecimal digits, so the name is
    /// a plain name.
    pub(crate) fn name(company: &CompanyId) -> String {
        format!("records_{}", company.as_str())
    }

    /// The key that opens `company`'s vault, for one request.
    pub(crate) fn key(&self, company: &CompanyId) -> Result<VaultKey> {
        Ok(self.keys.company(company.as_str())?)
    }

    /// Declare `company`'s vault and its sealed field, before the company's rows exist: a create that fails after
    /// this leaves an empty vault that nothing names, never a company without its vault.
    pub(crate) async fn declare(&self, place: &Place, company: &CompanyId) -> Result<()> {
        let vault = Self::name(company);
        self.database
            .declare_keyed_vault(place, &vault, &self.key(company)?)
            .await?;
        // The name was built from a checked company id above; it is grammar and cannot be bound.
        self.database
            .run_at(
                place,
                &format!("DEFINE FIELD IF NOT EXISTS body ON {vault} TYPE string SECRET;"),
                Vec::new(),
            )
            .await?;
        Ok(())
    }
}
