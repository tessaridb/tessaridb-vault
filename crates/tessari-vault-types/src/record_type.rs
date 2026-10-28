//! A record's type, which the server sees in the clear: everything else about the record is sealed in the browser.

use std::fmt;

use crate::ids::RecordTypeId;
use crate::roles::Roles;

/// The built-in types, by the ids the browser's record documents carry, and `record-type`: the kind of record a
/// company-defined type's sealed definition is kept in.
const BUILT_IN: [&str; 12] = [
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
    "record-type",
];

/// The kind of record a company-defined type's definition is kept in.
const DEFINITION: &str = "record-type";

/// How a company-defined type's id is written.
const COMPANY_PREFIX: &str = "company:";

/// Why a text is not a record type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a record type")]
pub struct RecordTypeError;

/// A built-in record type, or one a company defined.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RecordType {
    /// One of the built-in types.
    BuiltIn(&'static str),
    /// A type the company defined, by its id.
    Company(RecordTypeId),
}

impl RecordType {
    /// The type `text` names: a built-in id, or `company:` and a type id.
    ///
    /// # Errors
    ///
    /// [`RecordTypeError`] for anything else.
    pub fn parse(text: &str) -> Result<Self, RecordTypeError> {
        if let Some(id) = text.strip_prefix(COMPANY_PREFIX) {
            return RecordTypeId::parse(id)
                .map(Self::Company)
                .map_err(|_| RecordTypeError);
        }
        BUILT_IN
            .iter()
            .find(|id| **id == text)
            .map(|id| Self::BuiltIn(id))
            .ok_or(RecordTypeError)
    }

    /// Whether this is the kind a company-defined type's definition is kept in.
    #[must_use]
    pub fn is_definition(&self) -> bool {
        *self == Self::BuiltIn(DEFINITION)
    }

    /// Who may create a record of a built-in type: every role, but only managers for a type definition. A company
    /// type's roles are the company's, kept with the type.
    #[must_use]
    pub fn built_in_roles(&self) -> Option<Roles> {
        match self {
            Self::BuiltIn(_) if self.is_definition() => Some(Roles::MANAGERS),
            Self::BuiltIn(_) => Some(Roles::ALL),
            Self::Company(_) => None,
        }
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn(id) => formatter.write_str(id),
            Self::Company(id) => write!(formatter, "{COMPANY_PREFIX}{id}"),
        }
    }
}

#[cfg(test)]
#[path = "record_type_tests.rs"]
mod tests;
