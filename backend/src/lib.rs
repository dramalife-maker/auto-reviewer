pub mod config;
pub mod dashboard;
pub mod db;
pub mod error;
pub mod executor;
pub mod identity;
pub mod mr_change_materials;
pub mod mr_reviews;
pub mod pending_items;
pub mod person_trends;
pub mod projects;
pub mod reports;
pub mod runs;
pub mod schedule;
pub mod server;
pub mod state;
pub mod summary;
pub mod worker;
pub mod worktree;

use std::time::Duration;

use tokio_util::sync::CancellationToken;

pub use error::{Error, Result};

/// Hard upper bound on coordinated shutdown: from the moment the process
/// receives Ctrl+C / SIGTERM, cleanup has this long to finish before the
/// process exits regardless of what is still in flight.
const SHUTDOWN_HARD_DEADLINE: Duration = Duration::from_secs(15);

pub async fn run() -> Result<()> {
    app_env::load();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = config::AppConfig::from_env()?;
    let addr = config.listen_addr();
    let state = init_app(config, true).await?;

    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    serve_with_graceful_shutdown(listener, state, wait_for_shutdown_signal).await
}

/// Drive `axum::serve` with coordinated graceful shutdown and the 15s hard
/// deadline, given a listener and app state. Split out from [`run`] so the
/// shutdown sequencing (HTTP drain races the hard deadline) is testable
/// without binding a real signal handler or initializing global tracing.
/// Exposed (not just `pub(crate)`) so integration tests in `tests/` can
/// exercise the real deadline/drain behavior end-to-end.
pub async fn serve_with_graceful_shutdown<F, Fut>(
    listener: tokio::net::TcpListener,
    state: state::AppState,
    signal: F,
) -> Result<()>
where
    F: FnOnce(CancellationToken) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let shutdown = state.shutdown.clone();
    let app = server::router(state);

    let serve = axum::serve(listener, app).with_graceful_shutdown(signal(shutdown.clone()));

    tokio::select! {
        result = serve => result?,
        _ = shutdown_hard_deadline(shutdown) => {
            tracing::warn!(
                "shutdown hard deadline ({}s) reached; exiting without waiting for HTTP drain",
                SHUTDOWN_HARD_DEADLINE.as_secs()
            );
        }
    }

    Ok(())
}

/// Wait for Ctrl+C (all platforms) or SIGTERM (Unix only), then cancel the
/// shared shutdown token so worker/scheduler/executor paths observe it.
/// Windows builds only ever see Ctrl+C — there is no SIGTERM to listen for.
async fn wait_for_shutdown_signal(shutdown: CancellationToken) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sigterm = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            Ok(sig) => sig,
            Err(err) => {
                tracing::error!("failed to install SIGTERM handler: {err}");
                std::future::pending::<()>().await;
                return;
            }
        };
        sigterm.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl+C; starting coordinated shutdown"),
        _ = terminate => tracing::info!("received SIGTERM; starting coordinated shutdown"),
    }

    shutdown.cancel();
}

/// Fires exactly [`SHUTDOWN_HARD_DEADLINE`] after the shutdown token is
/// cancelled, so the process exits even if cleanup (HTTP drain, in-flight
/// executor kill, DB finalize) has not finished.
async fn shutdown_hard_deadline(shutdown: CancellationToken) {
    shutdown.cancelled().await;
    tokio::time::sleep(SHUTDOWN_HARD_DEADLINE).await;
}

pub async fn build_app() -> Result<axum::Router> {
    app_env::load();

    let config = config::AppConfig::from_env()?;
    let state = init_app(config, false).await?;
    Ok(server::router(state))
}

/// Build full `AppState` (worker + scheduler started) without binding an
/// HTTP listener. Exposed for integration tests that need to drive
/// [`serve_with_graceful_shutdown`] directly against a real listener.
pub async fn build_app_state() -> Result<state::AppState> {
    app_env::load();

    let config = config::AppConfig::from_env()?;
    init_app(config, true).await
}

async fn init_app(config: config::AppConfig, start_worker: bool) -> Result<state::AppState> {
    let pool = db::init_pool(config.data_dir()).await?;
    summary::backfill_pending_items_if_needed(&pool).await?;
    projects::ensure_projects_loaded(
        &pool,
        config.data_dir(),
        config.projects_config_path(),
    )
    .await?;
    let resolved = projects::load_resolved_from_db(&pool).await?;
    worktree::provision_all(&pool, &resolved).await;

    let shutdown = CancellationToken::new();

    let worker = if start_worker {
        // Startup recovery MUST run before the worker begins dequeuing, and
        // only applies to a real process boot — `build_app()` builds a
        // router against a possibly-shared pool for tests/tools and must
        // not silently flip rows a test just set up.
        runs::recover_orphaned_running_projects(&pool).await?;
        let worker = worker::RunWorker::spawn(config.clone(), pool.clone(), shutdown.clone());
        schedule::start_scheduler(pool.clone(), worker.clone(), shutdown.clone()).await?;
        Some(worker)
    } else {
        None
    };

    Ok(state::AppState {
        config,
        pool,
        worker,
        shutdown,
    })
}
