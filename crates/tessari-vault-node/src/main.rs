//! The `tessaridb-vault` binary: the composition root. It reads the configuration, declares the store's shape,
//! ensures the configured platform admin, serves the API and owns graceful shutdown; it holds no logic of its own.

use std::net::SocketAddr;
use std::process::ExitCode;

use tessari_vault_api::{AppState, router};
use tessari_vault_constants::STORE_CONNECTIONS;
use tessari_vault_infrastructure::config::NodeConfig;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_observability::init_tracing;
use tessari_vault_storage::{Storage, migrate};
use tessari_vault_types::{Email, Name};
use tokio::signal::unix::{SignalKind, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let _guard = match init_tracing() {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!("tessaridb-vault: {error}");
            return ExitCode::FAILURE;
        }
    };
    match run().await {
        Ok(name) => {
            tracing::info!(signal = name, "tessaridb-vault stopped");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(%error, "tessaridb-vault cannot run");
            ExitCode::FAILURE
        }
    }
}

/// Why the node could not start or keep serving.
#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error(transparent)]
    Config(#[from] tessari_vault_infrastructure::Error),
    #[error(transparent)]
    Storage(#[from] tessari_vault_storage::Error),
    #[error("the platform admin's {0} is not valid")]
    PlatformAdmin(&'static str),
    #[error("cannot listen: {0}")]
    Listen(#[source] std::io::Error),
}

async fn run() -> Result<&'static str, StartError> {
    let config = NodeConfig::from_lookup(|key| std::env::var(key).ok())?;
    let place = Place::new(config.namespace.as_str(), config.database.as_str())?;
    let database = Database::new(config.store.clone(), STORE_CONNECTIONS)?;
    migrate(&database, &place).await?;
    let storage = Storage::new(&database, &place);
    if let Some((email, name)) = &config.platform_admin {
        let email = Email::parse(email).map_err(|_| StartError::PlatformAdmin("address"))?;
        let name = Name::parse(name).map_err(|_| StartError::PlatformAdmin("name"))?;
        storage
            .accounts
            .system_ensure_platform_admin(&email, &name)
            .await?;
    }
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(StartError::Listen)?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), listen = %config.listen, "tessaridb-vault serving");
    let (stopped, signalled) = tokio::sync::oneshot::channel();
    let app = router(AppState { storage }).into_make_service_with_connect_info::<SocketAddr>();
    let serving = axum::serve(listener, app).with_graceful_shutdown(async move {
        let name = shutdown_signal().await.unwrap_or("signal listener failed");
        let _ = stopped.send(name);
    });
    serving.await.map_err(StartError::Listen)?;
    Ok(signalled.await.unwrap_or("stopped"))
}

/// Wait for SIGINT or SIGTERM and name the one that arrived.
async fn shutdown_signal() -> std::io::Result<&'static str> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = interrupt.recv() => Ok("SIGINT"),
        _ = terminate.recv() => Ok("SIGTERM"),
    }
}
