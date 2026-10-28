//! A record's type, which the server sees in the clear: everything else about the record is sealed in the browser.

use std::fmt;

/// The built-in types, by the ids the browser's record documents carry.
const BUILT_IN: [&str; 11] = [
    "login",
    "secure-note",
    "ssh-key",
    "file",
    "payment-card",
    "identity",
    "server",
    "database",
    "api-credential",
    "wireless-network",
    "software-license",
];

/// Why a text is not a record type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a record type")]
pub struct RecordTypeError;

/// One of the built-in record types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordType(&'static str);

impl RecordType {
    /// The type `text` names.
    ///
    /// # Errors
    ///
    /// [`RecordTypeError`] unless `text` is a built-in type id.
    pub fn parse(text: &str) -> Result<Self, RecordTypeError> {
        BUILT_IN
            .iter()
            .find(|id| **id == text)
            .map(|id| Self(id))
            .ok_or(RecordTypeError)
    }

    /// The type's id.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[cfg(test)]
#[path = "record_type_tests.rs"]
mod tests;
