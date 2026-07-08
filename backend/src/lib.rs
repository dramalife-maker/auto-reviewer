pub mod config;
pub mod dashboard;
pub mod db;
pub mod error;
pub mod executor;
pub mod identity;
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

pub use error::{Error, Result};

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
    let app = server::router(state);

    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

pub async fn build_app() -> Result<axum::Router> {
    app_env::load();

    let config = config::AppConfig::from_env()?;
    let state = init_app(config, false).await?;
    Ok(server::router(state))
}

async fn init_app(config: config::AppConfig, start_worker: bool) -> Result<state::AppState> {
    let pool = db::init_pool(config.data_dir()).await?;
    let resolved =
        projects::load_from_yaml(&pool, config.data_dir(), &config.projects_config_path()).await?;
    worktree::provision_all(&pool, &resolved).await;

    let worker = if start_worker {
        let worker = worker::RunWorker::spawn(config.clone(), pool.clone());
        schedule::start_scheduler(pool.clone(), worker.clone()).await?;
        Some(worker)
    } else {
        None
    };

    Ok(state::AppState {
        config,
        pool,
        worker,
    })
}
