//! What every handler is given.

use std::sync::Arc;

use tessari_vault_infrastructure::config::PasskeyParty;
use tessari_vault_infrastructure::oidc::OidcProvider;
use tessari_vault_storage::Storage;

/// The server's shared state.
#[derive(Clone)]
pub struct AppState {
    /// The storage services.
    pub storage: Storage,
    /// The OpenID Connect issuer people may sign in through, when one is configured.
    pub oidc: Option<Arc<OidcProvider>>,
    /// The passkey relying party, when the server's public address is configured.
    pub passkeys: Option<PasskeyParty>,
}
