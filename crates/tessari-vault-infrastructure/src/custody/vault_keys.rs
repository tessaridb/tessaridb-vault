//! The keys that open each company's records vault in the store (KV(c), key-hierarchy worksheet §9).
//!
//! The node keeps only KVM, derived from the service key under its own label, so the service key can still be dropped
//! at start-up; a company's key is derived from KVM and the company id when a request needs it, presented with that
//! request, and never stored.

use std::fmt;

use tessari_vault_constants::SERVICE_KEY_BYTES;
use tessaridb_client::VaultKey;
use zeroize::Zeroizing;

use super::ServiceKey;
use super::service_key::expand;
use crate::Result;

/// KVM, held only in memory and cleared when dropped.
pub struct VaultKeys(Zeroizing<[u8; SERVICE_KEY_BYTES]>);

impl fmt::Debug for VaultKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VaultKeys(<redacted>)")
    }
}

impl VaultKeys {
    /// KVM, from the service key.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Custody`] when the derivation fails.
    pub fn new(service_key: &ServiceKey) -> Result<Self> {
        service_key.vault_master().map(Self)
    }

    /// KV(c): the key that opens the records vault of the company `company`.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Custody`] when the derivation fails.
    pub fn company(&self, company: &str) -> Result<VaultKey> {
        let derived = expand(self.0.as_slice(), &[company.as_bytes()])?;
        Ok(VaultKey::new(*derived))
    }
}

#[cfg(test)]
#[path = "vault_keys_tests.rs"]
mod tests;
