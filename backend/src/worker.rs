use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Notify, Semaphore};
use tracing::{error, info};

use crate::config::AppConfig;
use crate::executor::{ExecuteOutcome, execute_weekly_batch};
use crate::runs::{
    fetch_next_queued_run_project, finalize_run_if_complete, finish_run_project,
    load_schedule_settings, mark_run_project_running, RunProjectRow,
};
use crate::summary::ingest_project_summaries;
use crate::worktree::{supply_worktree, WorktreeKind};

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

/// Resolve the working directory the reviewer subprocess runs in.
///
/// In test-executor mode the raw `repo_path` is used (no real tree needed).
/// Otherwise the resident worktree of the project's first default branch is
/// supplied (created / fetched / reset). An unhealthy project or a supply
/// failure yields `Err(reason)` so the caller skips the subprocess.
pub async fn resolve_working_dir(
    pool: &SqlitePool,
    job: &RunProjectRow,
) -> Result<PathBuf, String> {
    if std::env::var("REVIEWER_EXECUTOR").is_ok() {
        return Ok(PathBuf::from(&job.repo_path));
    }

    let (is_git_repo, default_branch) = crate::projects::get_project(pool, &job.name)
        .await
        .map_err(|e| e.to_string())?;
    if is_git_repo == 0 {
        return Err("project is unhealthy or not provisioned".to_string());
    }
    let branch = default_branch.ok_or_else(|| "no default branch to review".to_string())?;

    supply_worktree(std::path::Path::new(&job.repo_path), &branch, WorktreeKind::Resident)
        .await
        .map_err(|e| e.to_string())
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

    let working_dir = match resolve_working_dir(pool, &job).await {
        Ok(dir) => dir,
        Err(reason) => {
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            info!(run_id = job.run_id, project = %project.name, "run project skipped: {reason}");
            return Ok(());
        }
    };

    let (outcome, duration_sec, error) =
        execute_weekly_batch(pool, config, job.run_id, &project, &working_dir, timeout_sec).await?;

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
