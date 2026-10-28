//! The names a sign-in's attempts are counted under: keyed digests, so the counters hold no address and no password,
//! and a copy of them cannot be checked against a list of addresses without the service key.
//!
//! Each part is written with its length before it, so no two lists of parts digest alike. A password is put in
//! Normalization Form C first, as it is for hashing.

use std::fmt;

use ring::hmac;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::ServiceKey;
use crate::Result;

/// The key the sign-in counters are named under.
pub struct SigninKeys {
    key: hmac::Key,
}

impl fmt::Debug for SigninKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SigninKeys(<redacted>)")
    }
}

impl SigninKeys {
    /// The keys under the sign-in key `service_key` derives.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Custody`] when the derivation fails.
    pub fn new(service_key: &ServiceKey) -> Result<Self> {
        let derived = service_key.signin_key()?;
        Ok(Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, derived.as_slice()),
        })
    }

    /// The account key of `email`: the same for every attempt at that address, known or not.
    #[must_use]
    pub fn account(&self, email: &str) -> String {
        self.digest(&[b"account", email.as_bytes()])
    }

    /// The credential-pair key of `email` and `password`.
    #[must_use]
    pub fn pair(&self, email: &str, password: &str) -> String {
        let normalised = Zeroizing::new(password.nfc().collect::<String>());
        self.digest(&[b"pair", email.as_bytes(), normalised.as_bytes()])
    }

    /// The keyed digest of `parts`, each preceded by its length, as lowercase hex.
    fn digest(&self, parts: &[&[u8]]) -> String {
        let mut context = hmac::Context::with_key(&self.key);
        for part in parts {
            let length = u64::try_from(part.len()).unwrap_or(u64::MAX);
            context.update(&length.to_be_bytes());
            context.update(part);
        }
        context
            .sign()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[cfg(test)]
#[path = "signin_keys_tests.rs"]
mod tests;
