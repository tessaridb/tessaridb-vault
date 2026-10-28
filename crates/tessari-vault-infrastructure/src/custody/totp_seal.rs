//! TOTP seeds sealed at rest under KT (AES-256-GCM, NIST SP 800-38D), each bound to its account and factor.
//!
//! A sealed seed is one format byte, a fresh random 96-bit nonce, then the ciphertext with its tag. The associated
//! data is the format byte, the account and the factor, so a sealed seed copied onto another account's row, or onto
//! another factor of the same account, does not open. Seeds are sealed once per enrolment, far below the 2^32
//! random-nonce bound of SP 800-38D §8.3.

use std::fmt;

use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

use super::ServiceKey;
use crate::{Error, Result};

/// The first byte of every sealed seed: AES-256-GCM under KT, this layout. Anything else does not open.
const FORMAT: u8 = 1;

/// The key TOTP seeds are sealed under. Its bytes never leave this type.
pub struct TotpSeal {
    key: LessSafeKey,
    random: SystemRandom,
}

impl fmt::Debug for TotpSeal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TotpSeal(<redacted>)")
    }
}

const fn custody(reason: &'static str) -> Error {
    Error::Custody { reason }
}

/// What a sealed seed is bound to; each part is written with its length first, so no two bindings coincide.
fn binding(account: &str, factor: &str) -> Vec<u8> {
    let mut bound = vec![FORMAT];
    for part in [account, factor] {
        bound.extend_from_slice(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
        bound.extend_from_slice(part.as_bytes());
    }
    bound
}

impl TotpSeal {
    /// The seal under the KT `service_key` derives.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the key cannot be derived.
    pub fn new(service_key: &ServiceKey) -> Result<Self> {
        let derived = service_key.totp_key()?;
        let unbound = UnboundKey::new(&AES_256_GCM, derived.as_slice())
            .map_err(|_| custody("the TOTP sealing key is not usable"))?;
        Ok(Self {
            key: LessSafeKey::new(unbound),
            random: SystemRandom::new(),
        })
    }

    /// `seed` sealed for `factor` of `account`.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the random source or the cipher fails.
    pub fn seal(&self, account: &str, factor: &str, seed: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0_u8; NONCE_LEN];
        self.random
            .fill(&mut nonce)
            .map_err(|_| custody("the random source failed"))?;
        let mut sealed = seed.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(binding(account, factor)),
                &mut sealed,
            )
            .map_err(|_| custody("sealing failed"))?;
        let mut framed = vec![FORMAT];
        framed.extend_from_slice(&nonce);
        framed.extend(sealed);
        Ok(framed)
    }

    /// The seed [`TotpSeal::seal`] sealed for `factor` of `account`.
    ///
    /// # Errors
    ///
    /// [`Error::Custody`] when the format byte is not ours, the input is too short, or it was sealed under another
    /// key, for another account or factor, or altered.
    pub fn open(&self, account: &str, factor: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let refused = || custody("a sealed TOTP seed that does not open here");
        let (format, rest) = sealed.split_first().ok_or_else(refused)?;
        if *format != FORMAT {
            return Err(refused());
        }
        let (nonce, ciphertext) = rest.split_first_chunk::<NONCE_LEN>().ok_or_else(refused)?;
        let mut opened = Zeroizing::new(ciphertext.to_vec());
        let length = self
            .key
            .open_in_place(
                Nonce::assume_unique_for_key(*nonce),
                Aad::from(binding(account, factor)),
                &mut opened,
            )
            .map_err(|_| refused())?
            .len();
        opened.truncate(length);
        Ok(opened)
    }
}

#[cfg(test)]
#[path = "totp_seal_tests.rs"]
mod tests;
