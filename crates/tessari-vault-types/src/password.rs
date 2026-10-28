//! A new sign-in password, checked once where it enters, held so it cannot print and is cleared when dropped.

use std::fmt;

use tessari_vault_constants::{PASSWORD_MAX_CHARS, PASSWORD_MIN_CHARS};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::Email;

/// Why a new password was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    /// Fewer than 8 or more than 256 characters.
    #[error("a password is {PASSWORD_MIN_CHARS} to {PASSWORD_MAX_CHARS} characters")]
    Length,
    /// It holds a control character.
    #[error("a password holds no control characters")]
    Control,
    /// It is the account's address, or contains its address or the part before the `@`.
    #[error("a password does not contain the account's address")]
    Context,
}

/// A sign-in password that passed the policy: NIST SP 800-63B-4 §3.1.1, with a second factor always required, so
/// eight characters at least, every printable character and space allowed, no composition rules. Characters are
/// counted after Normalization Form C, the form it is hashed in.
pub struct Password(Zeroizing<String>);

impl fmt::Debug for Password {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Password(<redacted>)")
    }
}

impl Password {
    /// `text` as the password of the account at `email`.
    ///
    /// # Errors
    ///
    /// [`PasswordError`] naming the rule it breaks.
    pub fn parse(text: &str, email: &Email) -> Result<Self, PasswordError> {
        let normalised = Zeroizing::new(text.nfc().collect::<String>());
        let count = normalised.chars().count();
        if !(PASSWORD_MIN_CHARS..=PASSWORD_MAX_CHARS).contains(&count) {
            return Err(PasswordError::Length);
        }
        if normalised.chars().any(char::is_control) {
            return Err(PasswordError::Control);
        }
        let lowered = Zeroizing::new(normalised.to_lowercase());
        let address = email.as_str();
        let local = address.split_once('@').map_or(address, |(local, _)| local);
        if lowered.contains(address) || (local.chars().count() >= 3 && lowered.contains(local)) {
            return Err(PasswordError::Context);
        }
        Ok(Self(Zeroizing::new(text.to_owned())))
    }

    /// The password as typed, for the verifier to hash; the one place its text is reached.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[path = "password_tests.rs"]
mod tests;
