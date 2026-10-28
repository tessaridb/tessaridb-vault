//! The errors of `tessari-vault-infrastructure`.

/// A failure in `tessari-vault-infrastructure`. Variants arrive with the code that can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {}

/// The result of a `tessari-vault-infrastructure` call.
pub type Result<T> = std::result::Result<T, Error>;
