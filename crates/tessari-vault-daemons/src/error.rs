//! The errors of `tessari-vault-daemons`.

/// A failure in `tessari-vault-daemons`. Variants arrive with the code that can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {}

/// The result of a `tessari-vault-daemons` call.
pub type Result<T> = std::result::Result<T, Error>;
