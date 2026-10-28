//! The unlock derivation: unlock password and per-user salt → Argon2id → the 256-bit master key, which never leaves
//! the browser and only feeds the HKDF subkeys (key-hierarchy worksheet, KH-L1).

use argon2::{Algorithm, Argon2, Params, Version};
use tessari_vault_constants::{MASTER_KEY_BYTES, UNLOCK_PASSWORD_MAX_BYTES, UNLOCK_SALT_BYTES};
use tessari_vault_types::KdfParams;
use zeroize::Zeroizing;

use crate::{Error, Result};

/// The master key derived from the unlock password. It does not print, and it is cleared when dropped.
pub struct MasterKey(Zeroizing<[u8; MASTER_KEY_BYTES]>);

impl MasterKey {
    /// The key's bytes, for the derivation of its subkeys only.
    #[must_use]
    pub fn expose(&self) -> &[u8; MASTER_KEY_BYTES] {
        &self.0
    }
}

impl core::fmt::Debug for MasterKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("MasterKey(redacted)")
    }
}

/// Derive the master key from the unlock password with the user's own salt and parameters.
///
/// # Errors
///
/// [`Error::PasswordTooLong`] above `UNLOCK_PASSWORD_MAX_BYTES`; [`Error::Derivation`] when Argon2id refuses.
pub fn derive_master_key(
    password: &[u8],
    salt: &[u8; UNLOCK_SALT_BYTES],
    params: KdfParams,
) -> Result<MasterKey> {
    if password.len() > UNLOCK_PASSWORD_MAX_BYTES {
        return Err(Error::PasswordTooLong);
    }
    let costs = Params::new(
        params.memory_kib(),
        params.passes(),
        params.lanes(),
        Some(MASTER_KEY_BYTES),
    )
    .map_err(|_| Error::Derivation)?;
    let mut key = Zeroizing::new([0_u8; MASTER_KEY_BYTES]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, costs)
        .hash_password_into(password, salt, key.as_mut())
        .map_err(|_| Error::Derivation)?;
    Ok(MasterKey(key))
}

#[cfg(test)]
#[path = "unlock_tests.rs"]
mod tests;
