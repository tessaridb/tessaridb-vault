//! The errors of `tessari-vault-core`.

/// A failure in `tessari-vault-core`. Variants arrive with the code that can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {}

/// The result of a `tessari-vault-core` call.
pub type Result<T> = std::result::Result<T, Error>;
