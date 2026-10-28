//! The errors of `tessari-vault-infrastructure`.

use tessaridb_client::RefusalClass;

/// A failure reaching or configuring the store, or drawing randomness.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The store could not be reached, or the transport failed mid-request.
    #[error("the store is unavailable: {message}")]
    Unavailable {
        /// What the transport reported.
        message: String,
    },
    /// The store refused the statement.
    #[error("{message}")]
    Refused {
        /// The node's own words.
        message: String,
        /// What the node says to do about it; `None` from a node too old to send one.
        class: Option<RefusalClass>,
    },
    /// A setting is missing or not of the shape it must have.
    #[error("{field}: {reason}")]
    InvalidConfig {
        /// The setting at fault.
        field: &'static str,
        /// What it must be instead.
        reason: &'static str,
    },
    /// A namespace or database name is not a plain name.
    #[error("not a plain name: {name:?}")]
    InvalidName {
        /// The value offered as a name.
        name: String,
    },
    /// The operating system's random source failed.
    #[error("no randomness is available")]
    Randomness,
}

impl Error {
    /// Whether the node asked for the request to be tried again (`retry` class).
    #[must_use]
    pub const fn is_retry(&self) -> bool {
        matches!(
            self,
            Self::Refused {
                class: Some(RefusalClass::Retry),
                ..
            }
        )
    }
}

/// The result of a `tessari-vault-infrastructure` call.
pub type Result<T> = std::result::Result<T, Error>;
