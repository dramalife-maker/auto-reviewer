use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Notify, Semaphore};
use tracing::{error, info};

use crate::config::AppConfig;
use crate::executor::{ExecuteOutcome, execute_weekly_batch};
use crate::runs::{
    fetch_next_queued_run_project, finalize_run_if_complete, finish_run_project,
    load_schedule_settings, mark_run_project_running,
};
use crate::summary::ingest_project_summaries;

#[derive(Clone)]
pub struct RunWorker {
    pool: SqlitePool,
    config: AppConfig,
    notify: Arc<Notify>,
}

impl RunWorker {
    pub fn spawn(config: AppConfig, pool: SqlitePool) -> Arc<Self> {
        let worker = Arc::new(Self {
            pool,
            config,
            notify: Arc::new(Notify::new()),
        });
        let loop_worker = worker.clone();
        tokio::spawn(async move {
            loop_worker.run_loop().await;
        });
        worker
    }

    pub fn wake(&self) {
        self.notify.notify_one();
    }

    async fn run_loop(&self) {
        loop {
            self.notify.notified().await;
            if let Err(err) = self.drain_queue().await {
                error!("run worker error: {err}");
            }
        }
    }

    pub async fn drain_queue(&self) -> crate::Result<()> {
        let settings = load_schedule_settings(&self.pool).await?;
        let semaphore = Arc::new(Semaphore::new(settings.max_concurrency.max(1) as usize));
        let mut handles = Vec::new();

        while let Some(job) = fetch_next_queued_run_project(&self.pool).await? {
            let permit = semaphore.clone().acquire_owned().await.expect("semaphore");
            let pool = self.pool.clone();
            let config = self.config.clone();
            let timeout_sec = settings.per_project_timeout_sec.max(1) as u64;

            handles.push(tokio::spawn(async move {
                let result = process_run_project(&pool, &config, job, timeout_sec).await;
                drop(permit);
                result
            }));
        }

        for handle in handles {
            if let Err(err) = handle.await {
                error!("run project task join error: {err}");
            }
        }

        Ok(())
    }
}

pub async fn process_run_project(
    pool: &SqlitePool,
    config: &AppConfig,
    job: crate::runs::RunProjectRow,
    timeout_sec: u64,
) -> crate::Result<()> {
    mark_run_project_running(pool, job.id).await?;

    let project = crate::runs::ProjectRow {
        id: job.project_id,
        name: job.name.clone(),
        repo_path: job.repo_path.clone(),
    };

    let (outcome, duration_sec, error) =
        execute_weekly_batch(config, job.run_id, &project, timeout_sec).await?;

    let state = match outcome {
        ExecuteOutcome::Success => {
            ingest_project_summaries(
                pool,
                config.data_dir(),
                &project.name,
                project.id,
                job.run_id,
            )
            .await?;
            "done"
        }
        ExecuteOutcome::SkippedTimeout => "skipped_timeout",
        ExecuteOutcome::Failed => "failed",
    };

    finish_run_project(pool, job.id, state, duration_sec, error.as_deref()).await?;
    finalize_run_if_complete(pool, job.run_id).await?;

    info!(
        run_id = job.run_id,
        project = %project.name,
        state,
        "run project finished"
    );

    Ok(())
}
