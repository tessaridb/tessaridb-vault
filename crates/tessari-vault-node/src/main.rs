//! The `tessaridb-vault` binary: the composition root. It wires tracing and the runtime and owns graceful shutdown; it
//! holds no logic of its own. The listeners, storage and workers join it as their crates gain them.

use std::process::ExitCode;

use tessari_vault_observability::init_tracing;
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
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "tessaridb-vault started"
    );
    match shutdown_signal().await {
        Ok(name) => {
            tracing::info!(signal = name, "tessaridb-vault stopped");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(%error, "cannot listen for the shutdown signals");
            ExitCode::FAILURE
        }
    }
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
