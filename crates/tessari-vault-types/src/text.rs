//! Text a person supplies: an email address and a display name, each checked once where it enters.

use std::fmt;

/// The longest email address accepted, in bytes (RFC 5321 §4.5.3.1.3 path limit).
const EMAIL_MAX_BYTES: usize = 254;
/// The longest display name accepted, in characters.
const NAME_MAX_CHARS: usize = 120;

/// Why a text was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// Not an address of the form `local@domain`.
    #[error("not an email address")]
    Email,
    /// Empty, too long, or holding a control character.
    #[error("a name is 1 to {NAME_MAX_CHARS} characters with no control characters")]
    Name,
}

/// An email address, trimmed and lowercased so one person has one address.
///
/// Lowercasing the local part is a product decision: the address identifies a person here and two spellings of one
/// mailbox must not become two accounts. Nothing is sent to it in this version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

impl Email {
    /// The address `text` names.
    ///
    /// # Errors
    ///
    /// [`TextError::Email`] when it is longer than 254 bytes, holds whitespace or a control character, or is not
    /// exactly one `@` between a non-empty local part and a domain holding a dot.
    pub fn parse(text: &str) -> Result<Self, TextError> {
        let address = text.trim().to_lowercase();
        let shaped = address.len() <= EMAIL_MAX_BYTES
            && !address
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
            && matches!(
                address.split_once('@'),
                Some((local, domain))
                    if !local.is_empty()
                        && !domain.contains('@')
                        && domain.contains('.')
                        && !domain.starts_with('.')
                        && !domain.ends_with('.')
            );
        if shaped {
            Ok(Self(address))
        } else {
            Err(TextError::Email)
        }
    }

    /// The address as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A display name for a person, a company, a department or a team.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(String);

impl Name {
    /// The name `text` holds, trimmed.
    ///
    /// # Errors
    ///
    /// [`TextError::Name`] when it is empty after trimming, longer than 120 characters, or holds a control character.
    pub fn parse(text: &str) -> Result<Self, TextError> {
        let name = text.trim();
        let count = name.chars().count();
        if count == 0 || count > NAME_MAX_CHARS || name.chars().any(char::is_control) {
            return Err(TextError::Name);
        }
        Ok(Self(name.to_owned()))
    }

    /// The name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
