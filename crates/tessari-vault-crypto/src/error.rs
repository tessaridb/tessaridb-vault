//! The errors of `tessari-vault-crypto`. None carries key material or a password.

/// A failure of a cryptographic operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The unlock password is longer than `UNLOCK_PASSWORD_MAX_BYTES`.
    #[error("the unlock password is too long")]
    PasswordTooLong,
    /// The key derivation refused its inputs.
    #[error("the unlock key derivation failed")]
    Derivation,
}

/// The result of a `tessari-vault-crypto` call.
pub type Result<T> = std::result::Result<T, Error>;
