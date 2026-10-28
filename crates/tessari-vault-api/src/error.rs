//! The errors of `tessari-vault-api`.

/// A failure in `tessari-vault-api`. Variants arrive with the code that can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {}

/// The result of a `tessari-vault-api` call.
pub type Result<T> = std::result::Result<T, Error>;
