use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::AppConfig;
use crate::worker::RunWorker;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub pool: SqlitePool,
    pub worker: Option<Arc<RunWorker>>,
}
