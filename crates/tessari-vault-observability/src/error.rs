//! What installing the subscriber can fail with.

/// A failure to install this process's subscriber.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The process already has a subscriber; a second one would never receive an event.
    #[error("a tracing subscriber is already installed in this process")]
    AlreadyInstalled,
}

/// The result of an observability call.
pub type Result<T> = std::result::Result<T, Error>;
