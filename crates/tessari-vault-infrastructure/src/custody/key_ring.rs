//! The service key's epochs: the file holds one key, which is epoch 1, or one `<epoch> <64 hex>`
//! line per epoch in ascending order, and the highest epoch is current. Every value derived from the service key
//! names the epoch it was made under, so a new epoch can be added and the old one retired once nothing names it.

use std::fmt;
use std::path::Path;

use super::ServiceKey;
use super::service_key::{read_private, refuse};
use crate::Result;

/// Every epoch of the service key the file holds.
pub struct KeyRing {
    /// The current epoch: the highest, so a ring always has one.
    current: (u32, ServiceKey),
    /// The epochs before it, ascending.
    older: Vec<(u32, ServiceKey)>,
}

impl fmt::Debug for KeyRing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "KeyRing(epochs {:?}, <redacted>)", self.epochs())
    }
}

impl KeyRing {
    /// Read the ring from `path`.
    ///
    /// # Errors
    ///
    /// [`crate::Error::InvalidConfig`] for `TESSARIDB_VAULT_SERVICE_KEY_FILE` when the file cannot be read, is
    /// readable by anyone else, or is not one key or ascending numbered keys from epoch 1 up.
    pub fn load(path: &Path) -> Result<Self> {
        let text = read_private(path)?;
        let lines: Vec<&[u8]> = text
            .strip_suffix(b"\n")
            .unwrap_or(&text)
            .split(|byte| *byte == b'\n')
            .collect();
        if let [single] = lines.as_slice()
            && !single.contains(&b' ')
        {
            return Ok(Self {
                current: (1, ServiceKey::parse(single)?),
                older: Vec::new(),
            });
        }
        let shape = "one key, or `<epoch> <64 hex>` lines from epoch 1 up in ascending order";
        let mut epochs: Vec<(u32, ServiceKey)> = Vec::with_capacity(lines.len());
        for line in lines {
            let mut parts = line.splitn(2, |byte| *byte == b' ');
            let (Some(number), Some(digits)) = (parts.next(), parts.next()) else {
                return Err(refuse(shape));
            };
            let epoch = std::str::from_utf8(number)
                .ok()
                .and_then(|number| number.parse::<u32>().ok())
                .filter(|epoch| *epoch >= 1)
                .ok_or(refuse(shape))?;
            if epochs.last().is_some_and(|(last, _)| *last >= epoch) {
                return Err(refuse(shape));
            }
            epochs.push((epoch, ServiceKey::parse(digits)?));
        }
        let current = epochs.pop().ok_or(refuse(shape))?;
        Ok(Self {
            current,
            older: epochs,
        })
    }

    /// The current epoch's number.
    #[must_use]
    pub fn current_epoch(&self) -> u32 {
        self.current.0
    }

    /// The current epoch's key.
    #[must_use]
    pub fn current(&self) -> &ServiceKey {
        &self.current.1
    }

    /// The key of `epoch`, when the ring still holds it.
    #[must_use]
    pub fn epoch(&self, epoch: u32) -> Option<&ServiceKey> {
        self.older
            .iter()
            .chain(std::iter::once(&self.current))
            .find(|(number, _)| *number == epoch)
            .map(|(_, key)| key)
    }

    /// The epochs the ring holds, ascending.
    #[must_use]
    pub fn epochs(&self) -> Vec<u32> {
        self.older
            .iter()
            .chain(std::iter::once(&self.current))
            .map(|(epoch, _)| *epoch)
            .collect()
    }
}

#[cfg(test)]
#[path = "key_ring_tests.rs"]
mod tests;
