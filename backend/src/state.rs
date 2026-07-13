use std::sync::Arc;

use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::worker::RunWorker;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub pool: SqlitePool,
    pub worker: Option<Arc<RunWorker>>,
    /// Root shutdown cancellation token. Cancelled once by the process's
    /// coordinated shutdown sequence; cloned into worker/scheduler/executor
    /// paths (including HTTP `agent-turn`) so in-flight work can race
    /// against it and fail fast instead of leaking subprocesses.
    pub shutdown: CancellationToken,
}
