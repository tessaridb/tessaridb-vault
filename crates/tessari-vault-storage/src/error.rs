//! The errors of `tessari-vault-storage`.

/// A failure in `tessari-vault-storage`. Variants arrive with the code that can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {}

/// The result of a `tessari-vault-storage` call.
pub type Result<T> = std::result::Result<T, Error>;
