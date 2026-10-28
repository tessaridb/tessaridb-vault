//! The service key (K_svc): 256 random bits in a file only the service user can read, and the keys derived from it.
//!
//! The file holds 64 hexadecimal characters (`openssl rand -hex 32`), optionally followed by one newline. It is read
//! once at start-up; a file anyone else can read, or of any other shape, refuses the start and names the variable,
//! never falling back to a generated key. Each derived key is HKDF-SHA-256 (RFC 5869) under a label of its own
//! (key-hierarchy worksheet §9), so no two uses share a key.

use std::fmt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use ring::hkdf;
use tessari_vault_constants::SERVICE_KEY_BYTES;
use zeroize::Zeroizing;

use crate::{Error, Result};

/// The variable that names the file, which is what a refusal names.
const VARIABLE: &str = "TESSARIDB_VAULT_SERVICE_KEY_FILE";

/// Permission bits that let anyone but the owner reach the file.
const OTHERS_MASK: u32 = 0o077;

/// The label of the outer HMAC key over sign-in password verifiers (PEP).
const PEPPER_LABEL: &[u8] = b"tessaridb-vault/v1/pepper";

/// The label of the key TOTP seeds are sealed under at rest (KT).
const TOTP_LABEL: &[u8] = b"tessaridb-vault/v1/totp";

/// The label of the key sign-in counters are named under, so the counters hold no address and no password.
const SIGNIN_LABEL: &[u8] = b"tessaridb-vault/v1/sign-in";

/// The label of the key every company's database-vault key is derived from (KVM).
const DB_VAULT_LABEL: &[u8] = b"tessaridb-vault/v1/db-vault";

/// The service key, held only in memory and cleared when dropped.
pub struct ServiceKey(Zeroizing<[u8; SERVICE_KEY_BYTES]>);

impl fmt::Debug for ServiceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceKey(<redacted>)")
    }
}

/// One SHA-256 block: the length every derived key is expanded to.
struct KeyLength;

impl hkdf::KeyType for KeyLength {
    fn len(&self) -> usize {
        SERVICE_KEY_BYTES
    }
}

const fn refuse(reason: &'static str) -> Error {
    Error::InvalidConfig {
        field: VARIABLE,
        reason,
    }
}

impl ServiceKey {
    /// Read the key from `path`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidConfig`] for `TESSARIDB_VAULT_SERVICE_KEY_FILE` when the file cannot be read, any group or
    /// other permission bit is set, or it is not 64 hexadecimal characters with at most one trailing newline.
    pub fn load(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path).map_err(|_| refuse("the file cannot be read"))?;
        if metadata.permissions().mode() & OTHERS_MASK != 0 {
            return Err(refuse(
                "readable by group or others; it must be mode 600 or 400",
            ));
        }
        let text =
            Zeroizing::new(std::fs::read(path).map_err(|_| refuse("the file cannot be read"))?);
        let digits = text.strip_suffix(b"\n").unwrap_or(&text);
        let shape = "64 hexadecimal characters, as `openssl rand -hex 32` writes";
        if digits.len() != SERVICE_KEY_BYTES.saturating_mul(2) {
            return Err(refuse(shape));
        }
        let mut key = Zeroizing::new([0_u8; SERVICE_KEY_BYTES]);
        let (pairs, _) = digits.as_chunks::<2>();
        for (byte, [high, low]) in key.iter_mut().zip(pairs) {
            let (Some(high), Some(low)) = (nibble(*high), nibble(*low)) else {
                return Err(refuse(shape));
            };
            *byte = high << 4 | low;
        }
        Ok(Self(key))
    }

    /// The pepper (PEP): the HMAC key applied to a password before Argon2id.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the derivation fails.
    pub fn pepper(&self) -> Result<Zeroizing<[u8; SERVICE_KEY_BYTES]>> {
        self.derive(PEPPER_LABEL)
    }

    /// The TOTP sealing key (KT): the AES-256-GCM key TOTP seeds are sealed under at rest.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the derivation fails.
    pub fn totp_key(&self) -> Result<Zeroizing<[u8; SERVICE_KEY_BYTES]>> {
        self.derive(TOTP_LABEL)
    }

    /// The sign-in key: the HMAC key sign-in throttle counters are named under.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the derivation fails.
    pub fn signin_key(&self) -> Result<Zeroizing<[u8; SERVICE_KEY_BYTES]>> {
        self.derive(SIGNIN_LABEL)
    }

    /// The key every company's database-vault key is derived from (KVM), for [`super::VaultKeys`].
    pub(super) fn vault_master(&self) -> Result<Zeroizing<[u8; SERVICE_KEY_BYTES]>> {
        self.derive(DB_VAULT_LABEL)
    }

    fn derive(&self, label: &[u8]) -> Result<Zeroizing<[u8; SERVICE_KEY_BYTES]>> {
        expand(self.0.as_slice(), &[label])
    }
}

/// HKDF-SHA-256 of `key` with an empty salt and `info`, one key long.
pub(super) fn expand(key: &[u8], info: &[&[u8]]) -> Result<Zeroizing<[u8; SERVICE_KEY_BYTES]>> {
    let mut derived = Zeroizing::new([0_u8; SERVICE_KEY_BYTES]);
    hkdf::Salt::new(hkdf::HKDF_SHA256, &[])
        .extract(key)
        .expand(info, KeyLength)
        .and_then(|okm| okm.fill(derived.as_mut_slice()))
        .map_err(|_| Error::Custody {
            reason: "a key could not be derived from the service key",
        })?;
    Ok(derived)
}

/// The value of one hexadecimal digit, either case.
const fn nibble(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit.wrapping_sub(b'0')),
        b'a'..=b'f' => Some(digit.wrapping_sub(b'a').wrapping_add(10)),
        b'A'..=b'F' => Some(digit.wrapping_sub(b'A').wrapping_add(10)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "service_key_tests.rs"]
pub(super) mod tests;
