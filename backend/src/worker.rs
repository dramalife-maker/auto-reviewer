use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Notify, Semaphore};
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::executor::{execute_mr_review, execute_weekly_batch, parse_agent_session_id, ExecuteOutcome};
use crate::mr_reviews::{self, run_triage_script};
use crate::runs::{
    self, eligible_mrs_path, fetch_next_queued_run_project, finalize_run_if_complete,
    finish_run_project, load_mr_poll_project, load_schedule_settings, mark_run_project_running,
    write_mr_poll_manifest, RunProjectRow,
};
use crate::summary::ingest_project_summaries;
use crate::worktree::{provision_mr_worktree, supply_worktree, WorktreeKind};

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
            match handle.await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => error!("run project error: {err}"),
                Err(err) => error!("run project task join error: {err}"),
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
    config: &AppConfig,
    job: &RunProjectRow,
) -> Result<PathBuf, String> {
    if config.reviewer_executor().is_some() {
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
    if runs::is_mr_trigger(&job.trigger) {
        return process_mr_run_project(pool, config, job, timeout_sec).await;
    }

    mark_run_project_running(pool, job.id).await?;

    let project = crate::runs::ProjectRow {
        id: job.project_id,
        name: job.name.clone(),
        repo_path: job.repo_path.clone(),
    };

    let working_dir = match resolve_working_dir(pool, config, &job).await {
        Ok(dir) => dir,
        Err(reason) => {
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            info!(run_id = job.run_id, project = %project.name, "run project skipped: {reason}");
            return Ok(());
        }
    };

    let (outcome, duration_sec, error) =
        match execute_weekly_batch(pool, config, job.run_id, &project, &working_dir, timeout_sec)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                let reason = err.to_string();
                finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
                finalize_run_if_complete(pool, job.run_id).await?;
                error!(run_id = job.run_id, project = %project.name, "run project execute failed: {reason}");
                return Ok(());
            }
        };

    let (state, error) = match outcome {
        ExecuteOutcome::Success => {
            match ingest_project_summaries(
                pool,
                config.data_dir(),
                &project.name,
                project.id,
                job.run_id,
            )
            .await
            {
                Ok(()) => ("done", error),
                Err(err) => ("failed", Some(err.to_string())),
            }
        }
        ExecuteOutcome::SkippedTimeout => ("skipped_timeout", error),
        ExecuteOutcome::Failed => ("failed", error),
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

async fn process_mr_run_project(
    pool: &SqlitePool,
    config: &AppConfig,
    job: RunProjectRow,
    timeout_sec: u64,
) -> crate::Result<()> {
    mark_run_project_running(pool, job.id).await?;
    let started = std::time::Instant::now();

    let mr_project = match load_mr_poll_project(pool, job.project_id).await? {
        Some(project) => project,
        None => {
            finish_run_project(pool, job.id, "failed", 0, Some("project not found")).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            return Ok(());
        }
    };

    let resident_dir = match resolve_working_dir(pool, config, &job).await {
        Ok(dir) => dir,
        Err(reason) => {
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            info!(run_id = job.run_id, project = %job.name, "mr scan skipped: {reason}");
            return Ok(());
        }
    };

    let resident_str = resident_dir.display().to_string();
    let manifest_path = match write_mr_poll_manifest(
        config.data_dir(),
        job.run_id,
        &mr_project,
        &resident_str,
        None,
        None,
        Vec::new(),
    )
    .await
    {
        Ok(path) => path,
        Err(err) => {
            let reason = err.to_string();
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            return Ok(());
        }
    };

    if let Err(reason) = run_triage_script(config, &manifest_path, &resident_dir).await {
        finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
        finalize_run_if_complete(pool, job.run_id).await?;
        error!(run_id = job.run_id, project = %job.name, "mr triage failed: {reason}");
        return Ok(());
    }

    let eligible_path = eligible_mrs_path(config.data_dir(), job.run_id, job.project_id);
    let eligible_file = match mr_reviews::read_eligible_mrs(&eligible_path) {
        Ok(file) => file,
        Err(reason) => {
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            return Ok(());
        }
    };

    if eligible_file.eligible.is_empty() {
        let duration_sec = started.elapsed().as_secs() as i64;
        finish_run_project(pool, job.id, "done", duration_sec, None).await?;
        finalize_run_if_complete(pool, job.run_id).await?;
        info!(run_id = job.run_id, project = %job.name, "mr scan finished with no eligible MRs");
        return Ok(());
    }

    let force = job.mr_scan_force != 0;
    let blocked = match mr_reviews::load_inbox_blocked_rounds(pool, job.project_id).await {
        Ok(blocked) => blocked,
        Err(err) => {
            let reason = err.to_string();
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            error!(run_id = job.run_id, project = %job.name, "inbox gate load failed: {reason}");
            return Ok(());
        }
    };
    let (eligible_to_run, inbox_skipped) = mr_reviews::filter_eligible_by_inbox(
        &eligible_file.eligible,
        &blocked,
        force,
    );
    if let Err(reason) = mr_reviews::persist_inbox_gate_result(
        &eligible_path,
        &eligible_file,
        &eligible_to_run,
        &inbox_skipped,
    ) {
        warn!(
            run_id = job.run_id,
            project = %job.name,
            "failed to persist inbox gate result: {reason}"
        );
    }
    for skipped in &inbox_skipped {
        warn!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = skipped.mr.mr_iid,
            review_round = skipped.mr.review_round,
            skip_reason = %skipped.skip_reason,
            "mr inbox gate skipped eligible MR"
        );
    }

    if eligible_to_run.is_empty() {
        let duration_sec = started.elapsed().as_secs() as i64;
        finish_run_project(pool, job.id, "done", duration_sec, None).await?;
        finalize_run_if_complete(pool, job.run_id).await?;
        info!(
            run_id = job.run_id,
            project = %job.name,
            inbox_skipped = inbox_skipped.len(),
            "mr scan finished with no MRs to run after inbox gate"
        );
        return Ok(());
    }

    let agent = config.reviewer_agent();
    let draft_dir = runs::mr_poll_draft_dir(config.data_dir(), job.run_id, job.project_id);
    let mut had_failure = false;
    let mut had_timeout = false;

    for eligible in &eligible_to_run {
        // Test/custom executor mode mirrors weekly: skip real MR worktree supply.
        let mr_worktree = if config.reviewer_executor().is_some() {
            resident_dir.clone()
        } else {
            match provision_mr_worktree(
                std::path::Path::new(&job.repo_path),
                &eligible.source_branch,
            )
            .await
            {
                Ok(dir) => dir,
                Err(err) => {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        branch = %eligible.source_branch,
                        "mr worktree provision failed: {err}"
                    );
                    had_failure = true;
                    continue;
                }
            }
        };

        let prior_published_reviews = if eligible.review_round > 1 {
            match mr_reviews::load_prior_published_reviews(pool, job.project_id, eligible.mr_iid)
                .await
            {
                Ok(prior) => prior,
                Err(err) => {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        "failed to load prior published reviews: {err}"
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        let manifest_path = match write_mr_poll_manifest(
            config.data_dir(),
            job.run_id,
            &mr_project,
            &mr_worktree.display().to_string(),
            None,
            Some(eligible.mr_iid),
            prior_published_reviews,
        )
        .await
        {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    run_id = job.run_id,
                    project = %job.name,
                    mr_iid = eligible.mr_iid,
                    "mr manifest write failed: {err}"
                );
                had_failure = true;
                continue;
            }
        };

        let result = execute_mr_review(
            config,
            &mr_worktree,
            &manifest_path,
            timeout_sec,
            agent,
        )
        .await;

        // Agent may write drafts before exit/timeout; always attempt ingest.
        let session_id = parse_agent_session_id(&result.stdout, agent);
        if let Err(err) = mr_reviews::upsert_from_draft_dir(
            pool,
            job.project_id,
            &draft_dir,
            session_id.as_deref(),
            agent,
            force,
        )
        .await
        {
            warn!(
                run_id = job.run_id,
                project = %job.name,
                mr_iid = eligible.mr_iid,
                "mr draft ingest failed: {err}"
            );
            had_failure = true;
        }

        match result.outcome {
            ExecuteOutcome::Success => {
                if session_id.is_none() {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        "mr scan succeeded but no agent session id in stdout"
                    );
                }
            }
            ExecuteOutcome::SkippedTimeout => {
                had_timeout = true;
                info!(
                    run_id = job.run_id,
                    project = %job.name,
                    mr_iid = eligible.mr_iid,
                    "mr review timed out; ingested any drafts already on disk"
                );
            }
            ExecuteOutcome::Failed => {
                had_failure = true;
                if let Some(reason) = result.error.as_deref() {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        "mr review subprocess failed: {reason}"
                    );
                }
            }
        }
    }

    let duration_sec = started.elapsed().as_secs() as i64;
    let (state, error): (&str, Option<String>) = if had_timeout {
        ("skipped_timeout", None)
    } else if had_failure {
        ("failed", Some("one or more MR reviews failed".into()))
    } else {
        ("done", None)
    };

    finish_run_project(pool, job.id, state, duration_sec, error.as_deref()).await?;
    finalize_run_if_complete(pool, job.run_id).await?;

    info!(
        run_id = job.run_id,
        project = %job.name,
        state,
        eligible = eligible_to_run.len(),
        "mr scan finished"
    );

    Ok(())
}
