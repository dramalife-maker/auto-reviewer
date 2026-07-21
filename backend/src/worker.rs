use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use sqlx::SqlitePool;
use tokio::sync::{Notify, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::executor::{
    execute_mr_review, execute_weekly_batch, parse_agent_session_id, summarize_agent_output,
    ExecuteOutcome,
};
use crate::identity;
use crate::mr_change_materials::{
    mr_change_materials_dir, prepare_change_materials, write_stub_change_materials,
    DEFAULT_DIFF_MAX_BYTES,
};
use crate::mr_reviews::{self, run_triage_script};
use crate::runs::{
    self, eligible_mrs_path, fetch_next_queued_run_project, finalize_run_if_complete,
    finish_run_project, load_mr_poll_project, load_schedule_settings, mark_run_project_running,
    write_mr_poll_manifest, ManifestChangeMaterials, RunProjectRow, SHUTDOWN_INTERRUPTED_ERROR,
};
use crate::summary::ingest_project_summaries;
use crate::worktree::{provision_mr_worktree, supply_worktree, WorktreeKind};

#[derive(Clone)]
pub struct RunWorker {
    pool: SqlitePool,
    config: AppConfig,
    notify: Arc<Notify>,
    /// Root shutdown token. Cancelled by the process's coordinated shutdown
    /// sequence; once cancelled the worker stops dequeuing new `queued`
    /// `run_projects` rows and in-flight executor calls race against it.
    cancel: CancellationToken,
}

impl RunWorker {
    pub fn spawn(config: AppConfig, pool: SqlitePool, cancel: CancellationToken) -> Arc<Self> {
        let worker = Arc::new(Self {
            pool,
            config,
            notify: Arc::new(Notify::new()),
            cancel,
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
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("run worker stopping: shutdown cancellation observed");
                    return;
                }
                _ = self.notify.notified() => {
                    if self.cancel.is_cancelled() {
                        return;
                    }
                    if let Err(err) = self.drain_queue().await {
                        error!("run worker error: {err}");
                    }
                }
            }
        }
    }

    pub async fn drain_queue(&self) -> crate::Result<()> {
        let settings = load_schedule_settings(&self.pool).await?;
        let semaphore = Arc::new(Semaphore::new(settings.max_concurrency.max(1) as usize));
        let mut handles = Vec::new();

        loop {
            if self.cancel.is_cancelled() {
                break;
            }
            let permit = semaphore.clone().acquire_owned().await.expect("semaphore");
            if self.cancel.is_cancelled() {
                drop(permit);
                break;
            }
            let Some(job) = fetch_next_queued_run_project(&self.pool).await? else {
                drop(permit);
                break;
            };
            let pool = self.pool.clone();
            let config = self.config.clone();
            let timeout_sec = settings.per_project_timeout_sec.max(1) as u64;
            let cancel = self.cancel.clone();

            handles.push(tokio::spawn(async move {
                let result = process_run_project(&pool, &config, job, timeout_sec, cancel).await;
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
    cancel: CancellationToken,
) -> crate::Result<()> {
    if runs::is_mr_trigger(&job.trigger) {
        return process_mr_run_project(pool, config, job, timeout_sec, cancel).await;
    }

    mark_run_project_running(pool, job.id).await?;
    let total_started = Instant::now();

    let project = crate::runs::ProjectRow {
        id: job.project_id,
        name: job.name.clone(),
        repo_path: job.repo_path.clone(),
    };

    let stage_started = Instant::now();
    let working_dir = match resolve_working_dir(pool, config, &job).await {
        Ok(dir) => dir,
        Err(reason) => {
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            info!(run_id = job.run_id, project = %project.name, "run project skipped: {reason}");
            return Ok(());
        }
    };
    info!(
        run_id = job.run_id,
        project = %project.name,
        stage = "resolve_working_dir",
        elapsed_ms = stage_started.elapsed().as_millis() as u64,
        "weekly stage"
    );

    let stage_started = Instant::now();
    let (outcome, duration_sec, error) = match execute_weekly_batch(
        pool,
        config,
        job.run_id,
        &project,
        &working_dir,
        timeout_sec,
        cancel,
    )
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
    info!(
        run_id = job.run_id,
        project = %project.name,
        stage = "agent_execute",
        elapsed_ms = stage_started.elapsed().as_millis() as u64,
        duration_sec,
        outcome = ?outcome,
        "weekly stage"
    );

    let (state, error) = match outcome {
        ExecuteOutcome::Success => {
            let stage_started = Instant::now();
            let result = match ingest_project_summaries(
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
            };
            info!(
                run_id = job.run_id,
                project = %project.name,
                stage = "ingest_summaries",
                elapsed_ms = stage_started.elapsed().as_millis() as u64,
                "weekly stage"
            );
            result
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
        elapsed_ms = total_started.elapsed().as_millis() as u64,
        "run project finished"
    );

    Ok(())
}

async fn process_mr_run_project(
    pool: &SqlitePool,
    config: &AppConfig,
    job: RunProjectRow,
    timeout_sec: u64,
    cancel: CancellationToken,
) -> crate::Result<()> {
    mark_run_project_running(pool, job.id).await?;
    let started = Instant::now();

    let mr_project = match load_mr_poll_project(pool, job.project_id).await? {
        Some(project) => project,
        None => {
            finish_run_project(pool, job.id, "failed", 0, Some("project not found")).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            return Ok(());
        }
    };

    let stage_started = Instant::now();
    let resident_dir = match resolve_working_dir(pool, config, &job).await {
        Ok(dir) => dir,
        Err(reason) => {
            finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
            finalize_run_if_complete(pool, job.run_id).await?;
            info!(run_id = job.run_id, project = %job.name, "mr scan skipped: {reason}");
            return Ok(());
        }
    };
    info!(
        run_id = job.run_id,
        project = %job.name,
        stage = "resolve_working_dir",
        elapsed_ms = stage_started.elapsed().as_millis() as u64,
        "mr scan stage"
    );

    let resident_str = resident_dir.display().to_string();
    let stage_started = Instant::now();
    let manifest_path = match write_mr_poll_manifest(
        config.data_dir(),
        job.run_id,
        &mr_project,
        &resident_str,
        None,
        None,
        Vec::new(),
        None,
        None,
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
    info!(
        run_id = job.run_id,
        project = %job.name,
        stage = "write_triage_manifest",
        elapsed_ms = stage_started.elapsed().as_millis() as u64,
        "mr scan stage"
    );

    let stage_started = Instant::now();
    if let Err(reason) = run_triage_script(config, &manifest_path, &resident_dir).await {
        finish_run_project(pool, job.id, "failed", 0, Some(&reason)).await?;
        finalize_run_if_complete(pool, job.run_id).await?;
        error!(
            run_id = job.run_id,
            project = %job.name,
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            "mr triage failed: {reason}"
        );
        return Ok(());
    }
    info!(
        run_id = job.run_id,
        project = %job.name,
        stage = "triage",
        elapsed_ms = stage_started.elapsed().as_millis() as u64,
        "mr scan stage"
    );

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
        info!(
            run_id = job.run_id,
            project = %job.name,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "mr scan finished with no eligible MRs"
        );
        return Ok(());
    }

    let force = job.mr_scan_force != 0;
    let stage_started = Instant::now();
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
    info!(
        run_id = job.run_id,
        project = %job.name,
        stage = "inbox_gate",
        elapsed_ms = stage_started.elapsed().as_millis() as u64,
        eligible = eligible_to_run.len(),
        inbox_skipped = inbox_skipped.len(),
        "mr scan stage"
    );
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
            elapsed_ms = started.elapsed().as_millis() as u64,
            "mr scan finished with no MRs to run after inbox gate"
        );
        return Ok(());
    }

    let agent = config.reviewer_agent();
    let draft_dir = runs::mr_poll_draft_dir(config.data_dir(), job.run_id, job.project_id);
    let mut had_failure = false;
    let mut had_timeout = false;

    info!(
        run_id = job.run_id,
        project = %job.name,
        eligible = eligible_to_run.len(),
        "mr scan queue: processing eligible MRs sequentially (one agent at a time)"
    );

    for eligible in &eligible_to_run {
        if cancel.is_cancelled() {
            warn!(
                run_id = job.run_id,
                project = %job.name,
                mr_iid = eligible.mr_iid,
                "mr scan stopped before this MR: shutdown cancellation observed"
            );
            had_failure = true;
            break;
        }
        let mr_started = Instant::now();

        // Test/custom executor mode mirrors weekly: skip real MR worktree supply.
        let stage_started = Instant::now();
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
                        elapsed_ms = stage_started.elapsed().as_millis() as u64,
                        "mr worktree provision failed: {err}"
                    );
                    had_failure = true;
                    continue;
                }
            }
        };
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "provision_worktree",
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            "mr scan stage"
        );

        let stage_started = Instant::now();
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
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "load_prior_reviews",
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            prior_count = prior_published_reviews.len(),
            "mr scan stage"
        );

        let stage_started = Instant::now();
        let materials_dir = mr_change_materials_dir(
            config.data_dir(),
            job.run_id,
            job.project_id,
            eligible.mr_iid,
        );
        let material_paths = if config.reviewer_executor().is_some() {
            match write_stub_change_materials(&materials_dir) {
                Ok(paths) => paths,
                Err(err) => {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        elapsed_ms = stage_started.elapsed().as_millis() as u64,
                        "mr stub change materials failed: {err}"
                    );
                    had_failure = true;
                    continue;
                }
            }
        } else if eligible.target_branch.trim().is_empty() {
            warn!(
                run_id = job.run_id,
                project = %job.name,
                mr_iid = eligible.mr_iid,
                elapsed_ms = stage_started.elapsed().as_millis() as u64,
                "mr change materials skipped: empty target_branch"
            );
            had_failure = true;
            continue;
        } else {
            match prepare_change_materials(
                &mr_worktree,
                &eligible.target_branch,
                &materials_dir,
                DEFAULT_DIFF_MAX_BYTES,
            )
            .await
            {
                Ok(paths) => paths,
                Err(err) => {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        target_branch = %eligible.target_branch,
                        elapsed_ms = stage_started.elapsed().as_millis() as u64,
                        "mr change materials failed: {err}"
                    );
                    had_failure = true;
                    continue;
                }
            }
        };
        let change_materials = ManifestChangeMaterials::from_paths(&material_paths);
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "prepare_change_materials",
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            diff_truncated = material_paths.diff_truncated,
            "mr scan stage"
        );

        // Gate: every commit author on this MR must already be bound to a person.
        // Also keep the email list for observation folder naming: triage often
        // emits a GitLab username as `author_identity`, while bindings are on
        // `git_email` (see `resolve_observation_person_folder`).
        let stage_started = Instant::now();
        let commit_authors: Vec<String> = if config.reviewer_executor().is_none() {
            match crate::mr_change_materials::list_commit_authors(
                &mr_worktree,
                &eligible.target_branch,
            )
            .await
            {
                Ok(authors) => {
                    let mut unmatched = Vec::new();
                    for email in &authors {
                        match identity::resolve_person_by_email(pool, email).await {
                            Ok(Some(_)) => {}
                            Ok(None) => unmatched.push(email.clone()),
                            Err(err) => {
                                warn!(
                                    run_id = job.run_id,
                                    project = %job.name,
                                    mr_iid = eligible.mr_iid,
                                    email = %email,
                                    "identity lookup failed while checking commit authors: {err}"
                                );
                                unmatched.push(email.clone());
                            }
                        }
                    }
                    if !unmatched.is_empty() {
                        for email in &unmatched {
                            if let Err(err) = identity::record_unmatched_author(
                                pool,
                                identity::KIND_GIT_EMAIL,
                                email,
                                job.project_id,
                                1,
                            )
                            .await
                            {
                                warn!(
                                    run_id = job.run_id,
                                    project = %job.name,
                                    mr_iid = eligible.mr_iid,
                                    email = %email,
                                    "failed to record unmatched author: {err}"
                                );
                            }
                        }
                        warn!(
                            run_id = job.run_id,
                            project = %job.name,
                            mr_iid = eligible.mr_iid,
                            unmatched = %unmatched.join(", "),
                            elapsed_ms = stage_started.elapsed().as_millis() as u64,
                            "mr review skipped: unmatched commit authors"
                        );
                        had_failure = true;
                        continue;
                    }
                    authors
                }
                Err(err) => {
                    warn!(
                        run_id = job.run_id,
                        project = %job.name,
                        mr_iid = eligible.mr_iid,
                        elapsed_ms = stage_started.elapsed().as_millis() as u64,
                        "mr commit author check failed: {err}"
                    );
                    had_failure = true;
                    continue;
                }
            }
        } else {
            // Fixture/mock executor skips the gate; still try for folder naming.
            match crate::mr_change_materials::list_commit_authors(
                &mr_worktree,
                &eligible.target_branch,
            )
            .await
            {
                Ok(authors) => authors,
                Err(_) => Vec::new(),
            }
        };
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "check_commit_authors",
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            "mr scan stage"
        );

        let stage_started = Instant::now();
        let observation_person = match mr_reviews::resolve_observation_person_folder(
            pool,
            &eligible.author_identity,
            &commit_authors,
        )
        .await
        {
            Ok(Some(name)) => name,
            Ok(None) => {
                warn!(
                    run_id = job.run_id,
                    project = %job.name,
                    mr_iid = eligible.mr_iid,
                    author_identity = %eligible.author_identity,
                    commit_authors = %commit_authors.join(", "),
                    "mr review skipped: observation folder requires people.display_name (refusing author_identity fallback)"
                );
                had_failure = true;
                continue;
            }
            Err(err) => {
                warn!(
                    run_id = job.run_id,
                    project = %job.name,
                    mr_iid = eligible.mr_iid,
                    "mr review skipped: resolve observation person failed: {err}"
                );
                had_failure = true;
                continue;
            }
        };
        let manifest_path = match write_mr_poll_manifest(
            config.data_dir(),
            job.run_id,
            &mr_project,
            &mr_worktree.display().to_string(),
            None,
            Some(eligible.mr_iid),
            prior_published_reviews,
            Some(&change_materials),
            Some(observation_person.as_str()),
        )
        .await
        {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    run_id = job.run_id,
                    project = %job.name,
                    mr_iid = eligible.mr_iid,
                    elapsed_ms = stage_started.elapsed().as_millis() as u64,
                    "mr manifest write failed: {err}"
                );
                had_failure = true;
                continue;
            }
        };
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "write_mr_manifest",
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            "mr scan stage"
        );

        let stage_started = Instant::now();
        let result = execute_mr_review(
            config,
            &mr_worktree,
            &manifest_path,
            timeout_sec,
            agent,
            cancel.clone(),
        )
        .await;
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "agent_execute",
            elapsed_ms = stage_started.elapsed().as_millis() as u64,
            duration_sec = result.duration_sec,
            wait_ms = result.wait_ms,
            drain_ms = result.drain_ms,
            outcome = ?result.outcome,
            "mr scan stage"
        );

        let agent_out = summarize_agent_output(&result.stdout, &result.stderr);
        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stdout_bytes = agent_out.stdout_bytes,
            stderr_bytes = agent_out.stderr_bytes,
            stdout_lines = agent_out.stdout_lines,
            event_types = %agent_out.event_types,
            last_event_type = agent_out.last_event_type.as_deref().unwrap_or(""),
            last_assistant = agent_out.last_assistant_snippet.as_deref().unwrap_or(""),
            stdout_tail = %agent_out.stdout_tail,
            stderr_tail = %agent_out.stderr_tail,
            "mr agent output summary"
        );

        // Agent may write drafts before exit/timeout; always attempt ingest.
        let session_id = parse_agent_session_id(&result.stdout, agent);
        let stage_started = Instant::now();
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
                elapsed_ms = stage_started.elapsed().as_millis() as u64,
                "mr draft ingest failed: {err}"
            );
            had_failure = true;
        } else {
            info!(
                run_id = job.run_id,
                project = %job.name,
                mr_iid = eligible.mr_iid,
                stage = "draft_ingest",
                elapsed_ms = stage_started.elapsed().as_millis() as u64,
                "mr scan stage"
            );
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
                    elapsed_ms = mr_started.elapsed().as_millis() as u64,
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
                        elapsed_ms = mr_started.elapsed().as_millis() as u64,
                        "mr review subprocess failed: {reason}"
                    );
                }
            }
        }

        info!(
            run_id = job.run_id,
            project = %job.name,
            mr_iid = eligible.mr_iid,
            stage = "mr_total",
            elapsed_ms = mr_started.elapsed().as_millis() as u64,
            outcome = ?result.outcome,
            "mr scan stage"
        );
    }

    let duration_sec = started.elapsed().as_secs() as i64;
    let (state, error): (&str, Option<String>) = if cancel.is_cancelled() {
        ("failed", Some(SHUTDOWN_INTERRUPTED_ERROR.to_string()))
    } else if had_timeout {
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
        elapsed_ms = started.elapsed().as_millis() as u64,
        "mr scan finished"
    );

    Ok(())
}
