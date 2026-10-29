//! The keys that open each company's records vault in the store (KV(c), key-hierarchy worksheet §9).
//!
//! The node keeps only KVM, one per epoch of the service key, each derived from that epoch's key under its own label,
//! so the ring can still be dropped at start-up; a company's key is derived from the KVM of the epoch its vault is
//! keyed under and the company id when a request needs it, presented with that request, and never stored.

use std::fmt;

use tessari_vault_constants::SERVICE_KEY_BYTES;
use tessaridb_client::VaultKey;
use zeroize::Zeroizing;

use super::KeyRing;
use super::service_key::expand;
use crate::{Error, Result};

/// KVM for every epoch of the ring, held only in memory and cleared when dropped.
pub struct VaultKeys {
    current: u32,
    masters: Vec<(u32, Zeroizing<[u8; SERVICE_KEY_BYTES]>)>,
}

impl fmt::Debug for VaultKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VaultKeys(<redacted>)")
    }
}

impl VaultKeys {
    /// KVM for each epoch the ring holds.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when a derivation fails.
    pub fn new(ring: &KeyRing) -> Result<Self> {
        let masters = ring
            .epochs()
            .into_iter()
            .filter_map(|epoch| ring.epoch(epoch).map(|key| (epoch, key)))
            .map(|(epoch, key)| key.vault_master().map(|master| (epoch, master)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            current: ring.current_epoch(),
            masters,
        })
    }

    /// The epoch a vault declared now is keyed under.
    #[must_use]
    pub const fn current_epoch(&self) -> u32 {
        self.current
    }

    /// KV(c): the key that opens the records vault of the company `company`, keyed under `epoch`.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the ring no longer holds `epoch`, or the derivation fails.
    pub fn company(&self, epoch: u32, company: &str) -> Result<VaultKey> {
        let (_, master) = self.masters.iter().find(|(held, _)| *held == epoch).ok_or(
            Error::Custody {
                reason: "the service key ring does not hold the epoch this vault is keyed under",
            },
        )?;
        let derived = expand(master.as_slice(), &[company.as_bytes()])?;
        Ok(VaultKey::new(*derived))
    }
}

#[cfg(test)]
#[path = "vault_keys_tests.rs"]
mod tests;
