//! The errors of `tessari-vault-crypto`.

/// A failure in `tessari-vault-crypto`. Variants arrive with the code that can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {}

/// The result of a `tessari-vault-crypto` call.
pub type Result<T> = std::result::Result<T, Error>;
