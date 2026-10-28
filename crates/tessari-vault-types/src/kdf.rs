//! The cost of one user's unlock derivation, stored with the user and raised over time.

use tessari_vault_constants::{
    UNLOCK_KDF_MAX_LANES, UNLOCK_KDF_MAX_MEMORY_KIB, UNLOCK_KDF_MAX_PASSES, UNLOCK_KDF_MIN_LANES,
    UNLOCK_KDF_MIN_MEMORY_KIB, UNLOCK_KDF_MIN_PASSES,
};

/// Argon2id parameters inside the bounds every client enforces. A value of this type is never below the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    memory_kib: u32,
    passes: u32,
    lanes: u32,
}

/// Why a set of parameters was refused; each variant carries the value that was out of bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KdfParamsError {
    /// Memory in KiB outside `UNLOCK_KDF_MIN_MEMORY_KIB..=UNLOCK_KDF_MAX_MEMORY_KIB`.
    #[error("unlock memory {0} KiB is outside the allowed bounds")]
    MemoryOutOfBounds(u32),
    /// Passes outside `UNLOCK_KDF_MIN_PASSES..=UNLOCK_KDF_MAX_PASSES`.
    #[error("unlock passes {0} are outside the allowed bounds")]
    PassesOutOfBounds(u32),
    /// Lanes outside `UNLOCK_KDF_MIN_LANES..=UNLOCK_KDF_MAX_LANES`.
    #[error("unlock lanes {0} are outside the allowed bounds")]
    LanesOutOfBounds(u32),
}

impl KdfParams {
    /// Check the three costs against the bounds.
    ///
    /// # Errors
    ///
    /// The first cost found outside its bounds, as a [`KdfParamsError`].
    pub fn new(memory_kib: u32, passes: u32, lanes: u32) -> Result<Self, KdfParamsError> {
        if !(UNLOCK_KDF_MIN_MEMORY_KIB..=UNLOCK_KDF_MAX_MEMORY_KIB).contains(&memory_kib) {
            return Err(KdfParamsError::MemoryOutOfBounds(memory_kib));
        }
        if !(UNLOCK_KDF_MIN_PASSES..=UNLOCK_KDF_MAX_PASSES).contains(&passes) {
            return Err(KdfParamsError::PassesOutOfBounds(passes));
        }
        if !(UNLOCK_KDF_MIN_LANES..=UNLOCK_KDF_MAX_LANES).contains(&lanes) {
            return Err(KdfParamsError::LanesOutOfBounds(lanes));
        }
        Ok(Self {
            memory_kib,
            passes,
            lanes,
        })
    }

    /// Memory, in KiB.
    #[must_use]
    pub const fn memory_kib(self) -> u32 {
        self.memory_kib
    }

    /// Passes over the memory.
    #[must_use]
    pub const fn passes(self) -> u32 {
        self.passes
    }

    /// Lanes.
    #[must_use]
    pub const fn lanes(self) -> u32 {
        self.lanes
    }
}

#[cfg(test)]
#[path = "kdf_tests.rs"]
mod tests;
