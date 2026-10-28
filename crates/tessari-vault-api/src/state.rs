//! What every handler is given.

use tessari_vault_storage::Storage;

/// The server's shared state.
#[derive(Clone)]
pub struct AppState {
    /// The storage services.
    pub storage: Storage,
}
