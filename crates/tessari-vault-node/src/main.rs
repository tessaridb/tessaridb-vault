//! The `tessaridb-vault` binary: the composition root. It reads the configuration, declares the store's shape,
//! ensures the configured platform admin, serves the API and owns graceful shutdown; it holds no logic of its own.
//!
//! `tessaridb-vault enrolment-link` prints, instead of serving, a single-use link for the configured platform admin
//! while that account has no credential: the one way the first person gets in, and never written to a log.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use tessari_vault_api::{AppState, router};
use tessari_vault_constants::{
    PASSWORD_CONCURRENCY_DEFAULT, PASSWORD_HASH_ITERATIONS_FLOOR, PASSWORD_HASH_MEMORY_KIB_FLOOR,
    PASSWORD_HASH_PARALLELISM_FLOOR, STORE_CONNECTIONS,
};
use tessari_vault_infrastructure::config::NodeConfig;
use tessari_vault_infrastructure::custody::{
    PasswordCost, Passwords, ServiceKey, SigninKeys, TotpSeal,
};
use tessari_vault_infrastructure::oidc::OidcProvider;
use tessari_vault_infrastructure::tessaridb::{Database, Place};
use tessari_vault_observability::init_tracing;
use tessari_vault_storage::{Custody, Storage, migrate};
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
    #[error("unknown command {0:?}; the only one is `enrolment-link`")]
    Command(String),
    #[error("`enrolment-link` needs TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL and _NAME")]
    NoPlatformAdmin,
}

async fn run() -> Result<&'static str, StartError> {
    let command = std::env::args().nth(1);
    if let Some(other) = command.as_deref().filter(|name| *name != "enrolment-link") {
        return Err(StartError::Command(other.to_owned()));
    }
    let config = NodeConfig::from_lookup(|key| std::env::var(key).ok())?;
    // Read before anything else is reached: a missing or open key file refuses the start, naming the variable. The
    // key is dropped, and cleared, once the keys derived from it exist.
    let service_key = ServiceKey::load(&config.service_key_file)?;
    let cost = PasswordCost::new(
        PASSWORD_HASH_MEMORY_KIB_FLOOR,
        PASSWORD_HASH_ITERATIONS_FLOOR,
        PASSWORD_HASH_PARALLELISM_FLOOR,
    )?;
    let passwords = Passwords::new(&service_key, cost, PASSWORD_CONCURRENCY_DEFAULT)?;
    let seal = Arc::new(TotpSeal::new(&service_key)?);
    let signin_keys = SigninKeys::new(&service_key)?;
    drop(service_key);
    let place = Place::new(config.namespace.as_str(), config.database.as_str())?;
    let database = Database::new(config.store.clone(), STORE_CONNECTIONS)?;
    migrate(&database, &place).await?;
    let storage = Storage::new(
        &database,
        &place,
        Custody {
            passwords,
            seal,
            signin_keys,
        },
    );
    let mut platform_admin = None;
    if let Some((email, name)) = &config.platform_admin {
        let email = Email::parse(email).map_err(|_| StartError::PlatformAdmin("address"))?;
        let name = Name::parse(name).map_err(|_| StartError::PlatformAdmin("name"))?;
        let account = storage
            .accounts
            .system_ensure_platform_admin(&email, &name)
            .await?;
        platform_admin = Some((email, account));
    }
    if command.is_some() {
        let (email, account) = platform_admin.ok_or(StartError::NoPlatformAdmin)?;
        let token = storage.enrolments.system_issue(&account).await?;
        // Printed for the operator who ran the command, not logged: a log is read by more people than the link is for.
        println!(
            "enrolment token for {email}, valid for one use within 24 hours:\n{}",
            token.expose()
        );
        return Ok("enrolment-link");
    }
    // A configured issuer that cannot be discovered refuses the start rather than serving without it.
    let oidc = match &config.oidc {
        Some(issuer) => Some(Arc::new(
            OidcProvider::discover(&issuer.issuer, &issuer.client_id, &issuer.redirect).await?,
        )),
        None => None,
    };
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(StartError::Listen)?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), listen = %config.listen, "tessaridb-vault serving");
    let (stopped, signalled) = tokio::sync::oneshot::channel();
    let app = router(AppState {
        storage,
        oidc,
        passkeys: config.passkeys.clone(),
    })
    .into_make_service_with_connect_info::<SocketAddr>();
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
