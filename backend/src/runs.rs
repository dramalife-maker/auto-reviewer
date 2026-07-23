use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use sqlx::Row;
use tracing::warn;

use crate::identity::{self, ManifestAuthor};

pub fn is_mr_trigger(trigger: &str) -> bool {
    matches!(trigger, "mr_poll" | "manual_mr_poll")
}

pub const DEFAULT_MR_REVIEW_SKIP_LABELS: &[&str] =
    &["wip", "do-not-review", "no-ai-review"];

/// Error string stamped on `run_projects.error` when an in-flight row is
/// interrupted by the current process's coordinated shutdown.
pub const SHUTDOWN_INTERRUPTED_ERROR: &str = "interrupted by shutdown";

/// Error string stamped on `run_projects.error` when startup recovery finds
/// a `running` row orphaned by a previous process that never shut down
/// cleanly (e.g. `kill -9`).
pub const PREVIOUS_SHUTDOWN_INTERRUPTED_ERROR: &str = "interrupted by previous shutdown";

/// Terminal value for `runs.status` and `run_projects.state` when a user
/// cancels a run. Both columns are unconstrained TEXT, so no migration is
/// needed to introduce it. The claim query filters `r.status = 'running'`, so
/// setting a run to this value also closes its claim gate.
pub const CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: i64,
    pub name: String,
    pub repo_path: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MrPollProjectRow {
    pub id: i64,
    pub name: String,
    pub repo_path: String,
    pub mr_review_skip_labels: String,
    pub mr_review_require_label: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RunProjectRow {
    pub id: i64,
    pub run_id: i64,
    pub project_id: i64,
    pub name: String,
    pub repo_path: String,
    pub trigger: String,
    pub mr_scan_force: i64,
    /// NULL = process all resolved authors (batch semantics). Non-NULL = scope
    /// the weekly manifest to this single person (manual_person runs).
    pub person_id: Option<i64>,
}

pub fn parse_mr_scan_force(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1") | Some("true")
    )
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduleSettings {
    pub per_project_timeout_sec: i64,
    pub max_concurrency: i64,
}

pub async fn has_active_run_projects(pool: &sqlx::SqlitePool) -> crate::Result<bool> {
    let row = sqlx::query(
        "SELECT COUNT(*) FROM run_projects rp
         INNER JOIN runs r ON r.id = rp.run_id
         WHERE r.status = 'running' AND rp.state IN ('queued', 'running')",
    )
    .fetch_one(pool)
    .await
    .map_err(crate::Error::Database)?;
    Ok(row.get::<i64, _>(0) > 0)
}

pub async fn has_active_run_for_project(
    pool: &sqlx::SqlitePool,
    project_id: i64,
) -> crate::Result<bool> {
    let row = sqlx::query(
        "SELECT COUNT(*) FROM run_projects rp
         INNER JOIN runs r ON r.id = rp.run_id
         WHERE rp.project_id = ?
           AND r.status = 'running'
           AND rp.state IN ('queued', 'running')",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(crate::Error::Database)?;
    Ok(row.get::<i64, _>(0) > 0)
}

pub async fn create_manual_all_run(pool: &sqlx::SqlitePool) -> crate::Result<i64> {
    create_batch_run(pool, "manual_all").await
}

pub async fn create_manual_project_run(
    pool: &sqlx::SqlitePool,
    project_name: &str,
) -> crate::Result<i64> {
    if has_active_run_projects(pool).await? {
        return Err(crate::Error::RunConflict);
    }

    let project = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, repo_path FROM projects WHERE name = ?",
    )
    .bind(project_name)
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)?
    .ok_or(crate::Error::NotFound)?;

    let mut tx = pool.begin().await.map_err(crate::Error::Database)?;

    let result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_project', 'running', 1)",
    )
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;
    let run_id = result.last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project.id)
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;

    tx.commit().await.map_err(crate::Error::Database)?;
    Ok(run_id)
}

/// Enqueue a single project scoped to one person. Validates existence only
/// (project by name, person by id); whether the person has any activity in the
/// window is left to the worker — an empty window yields a no-op run, not an
/// error. Uses the same whole-system concurrency gate as the batch runs.
pub async fn create_manual_person_run(
    pool: &sqlx::SqlitePool,
    project_name: &str,
    person_id: i64,
) -> crate::Result<i64> {
    if has_active_run_projects(pool).await? {
        return Err(crate::Error::RunConflict);
    }

    let project = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, repo_path FROM projects WHERE name = ?",
    )
    .bind(project_name)
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)?
    .ok_or(crate::Error::NotFound)?;

    let person_exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM people WHERE id = ?")
            .bind(person_id)
            .fetch_optional(pool)
            .await
            .map_err(crate::Error::Database)?;
    if person_exists.is_none() {
        return Err(crate::Error::NotFound);
    }

    let mut tx = pool.begin().await.map_err(crate::Error::Database)?;

    let result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_person', 'running', 1)",
    )
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;
    let run_id = result.last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, person_id, state) VALUES (?, ?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project.id)
    .bind(person_id)
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;

    tx.commit().await.map_err(crate::Error::Database)?;
    Ok(run_id)
}

pub async fn create_manual_mr_scan_run(
    pool: &sqlx::SqlitePool,
    project_id: i64,
    force: bool,
) -> crate::Result<i64> {
    if has_active_run_for_project(pool, project_id).await? {
        return Err(crate::Error::RunConflict);
    }

    let project = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, repo_path FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)?
    .ok_or(crate::Error::NotFound)?;

    let mut tx = pool.begin().await.map_err(crate::Error::Database)?;

    let result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total, mr_scan_force) VALUES ('manual_mr_poll', 'running', 1, ?)",
    )
    .bind(i64::from(force))
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;
    let run_id = result.last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project.id)
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;

    tx.commit().await.map_err(crate::Error::Database)?;
    Ok(run_id)
}

pub async fn create_mr_poll_run(pool: &sqlx::SqlitePool) -> crate::Result<i64> {
    let mut tx = pool.begin().await.map_err(crate::Error::Database)?;

    let result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('mr_poll', 'running', 0)",
    )
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;
    let run_id = result.last_insert_rowid();

    let projects = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, repo_path FROM projects WHERE is_git_repo = 1 ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;

    let mut enqueued = 0_i64;
    for project in &projects {
        let active = sqlx::query(
            "SELECT COUNT(*) FROM run_projects rp
             INNER JOIN runs r ON r.id = rp.run_id
             WHERE rp.project_id = ?
               AND r.status = 'running'
               AND rp.state IN ('queued', 'running')",
        )
        .bind(project.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::Error::Database)?;
        if active.get::<i64, _>(0) > 0 {
            continue;
        }

        sqlx::query(
            "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
        )
        .bind(run_id)
        .bind(project.id)
        .execute(&mut *tx)
        .await
        .map_err(crate::Error::Database)?;
        enqueued += 1;
    }

    sqlx::query("UPDATE runs SET project_total = ? WHERE id = ?")
        .bind(enqueued)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::Error::Database)?;

    tx.commit().await.map_err(crate::Error::Database)?;
    Ok(run_id)
}

pub async fn get_run_trigger(
    pool: &sqlx::SqlitePool,
    run_id: i64,
) -> crate::Result<Option<String>> {
    sqlx::query_scalar("SELECT trigger FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .map_err(crate::Error::Database)
}

pub async fn load_mr_poll_project(
    pool: &sqlx::SqlitePool,
    project_id: i64,
) -> crate::Result<Option<MrPollProjectRow>> {
    sqlx::query_as::<_, MrPollProjectRow>(
        "SELECT id, name, repo_path, mr_review_skip_labels, mr_review_require_label
         FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)
}

pub async fn create_scheduled_run(pool: &sqlx::SqlitePool) -> crate::Result<i64> {
    create_batch_run(pool, "schedule").await
}

pub async fn create_batch_run(pool: &sqlx::SqlitePool, trigger: &str) -> crate::Result<i64> {
    if has_active_run_projects(pool).await? {
        return Err(crate::Error::RunConflict);
    }

    let mut tx = pool.begin().await.map_err(crate::Error::Database)?;

    let result = sqlx::query("INSERT INTO runs (trigger, status, project_total) VALUES (?, 'running', 0)")
        .bind(trigger)
        .execute(&mut *tx)
        .await
        .map_err(crate::Error::Database)?;
    let run_id = result.last_insert_rowid();

    let projects = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, repo_path FROM projects ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;

    for project in &projects {
        sqlx::query(
            "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
        )
        .bind(run_id)
        .bind(project.id)
        .execute(&mut *tx)
        .await
        .map_err(crate::Error::Database)?;
    }

    sqlx::query("UPDATE runs SET project_total = ? WHERE id = ?")
        .bind(projects.len() as i64)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::Error::Database)?;

    tx.commit().await.map_err(crate::Error::Database)?;
    Ok(run_id)
}

pub async fn fetch_next_queued_run_project(
    pool: &sqlx::SqlitePool,
) -> crate::Result<Option<RunProjectRow>> {
    // Claim with a single atomic UPDATE so drain_queue cannot spawn the same
    // queued row twice. A read-then-write transaction would take a read
    // snapshot first and fail with SQLITE_BUSY_SNAPSHOT on the upgrade whenever
    // a concurrent writer committed in between — busy_timeout never retries
    // that. One write statement has no snapshot to invalidate.
    loop {
        let claimed_id: Option<i64> = sqlx::query_scalar(
            "UPDATE run_projects
             SET state = 'running', started_at = datetime('now')
             WHERE id = (
                 SELECT rp.id
                 FROM run_projects rp
                 INNER JOIN runs r ON r.id = rp.run_id
                 WHERE r.status = 'running' AND rp.state = 'queued'
                 ORDER BY rp.id
                 LIMIT 1
             )
             RETURNING id",
        )
        .fetch_optional(pool)
        .await
        .map_err(crate::Error::Database)?;

        let Some(claimed_id) = claimed_id else {
            return Ok(None);
        };

        // RETURNING cannot yield the joined columns, so read them back. The row
        // is already claimed, so no other claimer can touch it.
        let row = sqlx::query_as::<_, RunProjectRow>(
            "SELECT rp.id, rp.run_id, rp.project_id, p.name, p.repo_path, r.trigger, r.mr_scan_force, rp.person_id
             FROM run_projects rp
             INNER JOIN projects p ON p.id = rp.project_id
             INNER JOIN runs r ON r.id = rp.run_id
             WHERE rp.id = ?",
        )
        .bind(claimed_id)
        .fetch_optional(pool)
        .await
        .map_err(crate::Error::Database)?;

        match row {
            Some(row) => return Ok(Some(row)),
            // The project (and its run_projects rows) was deleted between the
            // claim and the read-back; skip it and claim the next queued row.
            None => {
                warn!(
                    run_project_id = claimed_id,
                    "claimed run project vanished before read-back; skipping"
                );
            }
        }
    }
}

pub async fn finish_run_project(
    pool: &sqlx::SqlitePool,
    run_project_id: i64,
    state: &str,
    duration_sec: i64,
    error: Option<&str>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE run_projects
         SET state = ?, finished_at = datetime('now'), duration_sec = ?, error = ?
         WHERE id = ?",
    )
    .bind(state)
    .bind(duration_sec)
    .bind(error)
    .bind(run_project_id)
    .execute(pool)
    .await
    .map_err(crate::Error::Database)?;
    Ok(())
}

pub async fn finalize_run_if_complete(pool: &sqlx::SqlitePool, run_id: i64) -> crate::Result<()> {
    // A cancelled run is terminal. Projects still executing when the run was
    // cancelled will finish and call this; without the guard their completion
    // would overwrite `cancelled` with success/partial/failed.
    let current_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM runs WHERE id = ?")
            .bind(run_id)
            .fetch_optional(pool)
            .await
            .map_err(crate::Error::Database)?;
    if current_status.as_deref() == Some(CANCELLED) {
        return Ok(());
    }

    let row = sqlx::query(
        "SELECT
            SUM(CASE WHEN state IN ('queued', 'running') THEN 1 ELSE 0 END) AS pending,
            SUM(CASE WHEN state = 'skipped_timeout' THEN 1 ELSE 0 END) AS skipped,
            SUM(CASE WHEN state = 'failed' THEN 1 ELSE 0 END) AS failed
         FROM run_projects WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .map_err(crate::Error::Database)?;

    let pending: i64 = row.get("pending");
    if pending > 0 {
        return Ok(());
    }

    let skipped: i64 = row.get("skipped");
    let failed: i64 = row.get("failed");
    let status = if failed > 0 && skipped == 0 {
        "failed"
    } else if skipped > 0 {
        "partial"
    } else {
        "success"
    };

    sqlx::query(
        "UPDATE runs
         SET status = ?,
             finished_at = datetime('now'),
             project_skipped = ?,
             duration_sec = CAST((julianday(datetime('now')) - julianday(started_at)) * 86400 AS INTEGER)
         WHERE id = ?",
    )
    .bind(status)
    .bind(skipped)
    .bind(run_id)
    .execute(pool)
    .await
    .map_err(crate::Error::Database)?;

    Ok(())
}

/// Cancel a run: mark the run `cancelled` (closing its claim gate) and set
/// every still-`queued` project to `cancelled` so none is ever claimed. Running
/// projects are left for the per-run token-kill path to finalize as `cancelled`.
///
/// Returns `NotFound` when no such run exists and `RunConflict` when the run has
/// already reached a terminal status; in the conflict case no row is modified.
pub async fn cancel_run(pool: &sqlx::SqlitePool, run_id: i64) -> crate::Result<()> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .map_err(crate::Error::Database)?;
    let status = status.ok_or(crate::Error::NotFound)?;
    if status != "running" {
        return Err(crate::Error::RunConflict);
    }

    let mut tx = pool.begin().await.map_err(crate::Error::Database)?;

    // Guard against a finalize racing in between the check and the write.
    let updated = sqlx::query(
        "UPDATE runs
         SET status = ?,
             finished_at = datetime('now'),
             duration_sec = CAST((julianday(datetime('now')) - julianday(started_at)) * 86400 AS INTEGER)
         WHERE id = ? AND status = 'running'",
    )
    .bind(CANCELLED)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;
    if updated.rows_affected() == 0 {
        return Err(crate::Error::RunConflict);
    }

    sqlx::query(
        "UPDATE run_projects
         SET state = ?, finished_at = datetime('now')
         WHERE run_id = ? AND state = 'queued'",
    )
    .bind(CANCELLED)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(crate::Error::Database)?;

    tx.commit().await.map_err(crate::Error::Database)?;
    Ok(())
}

/// Startup recovery: fail every `run_projects` row left `running` by a
/// previous process that never shut down cleanly (e.g. `kill -9`), and
/// finalize each affected parent run. MUST run before the run worker begins
/// dequeuing so no worker ever races this recovery pass. `queued` rows are
/// left untouched so they can still be dequeued after this process starts.
pub async fn recover_orphaned_running_projects(pool: &sqlx::SqlitePool) -> crate::Result<()> {
    let run_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT run_id FROM run_projects WHERE state = 'running'",
    )
    .fetch_all(pool)
    .await
    .map_err(crate::Error::Database)?;

    if run_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "UPDATE run_projects
         SET state = 'failed', finished_at = datetime('now'), error = ?
         WHERE state = 'running'",
    )
    .bind(PREVIOUS_SHUTDOWN_INTERRUPTED_ERROR)
    .execute(pool)
    .await
    .map_err(crate::Error::Database)?;

    for run_id in run_ids {
        finalize_run_if_complete(pool, run_id).await?;
    }

    Ok(())
}

pub async fn load_schedule_settings(pool: &sqlx::SqlitePool) -> crate::Result<ScheduleSettings> {
    sqlx::query_as::<_, ScheduleSettings>(
        "SELECT per_project_timeout_sec, max_concurrency FROM schedule_config WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(crate::Error::Database)
}

pub async fn count_run_projects_by_state(
    pool: &sqlx::SqlitePool,
    run_id: i64,
    state: &str,
) -> crate::Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) FROM run_projects WHERE run_id = ? AND state = ?")
        .bind(run_id)
        .bind(state)
        .fetch_one(pool)
        .await
        .map_err(crate::Error::Database)?;
    Ok(row.get(0))
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunRow {
    pub id: i64,
    pub trigger: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_sec: Option<i64>,
    pub project_total: Option<i64>,
    pub project_skipped: i64,
    pub note: Option<String>,
}

pub async fn get_run(pool: &sqlx::SqlitePool, run_id: i64) -> crate::Result<Option<RunRow>> {
    sqlx::query_as::<_, RunRow>(
        "SELECT id, trigger, status, started_at, finished_at, duration_sec,
                project_total, project_skipped, note
         FROM runs WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)
}

#[derive(Debug, Clone)]
pub struct ListRunsFilter {
    pub limit: i64,
    pub offset: i64,
    pub trigger: Option<String>,
    pub status: Option<String>,
}

impl ListRunsFilter {
    pub fn from_query(
        limit: Option<i64>,
        offset: Option<i64>,
        trigger: Option<String>,
        status: Option<String>,
    ) -> crate::Result<Self> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);
        if limit < 1 || limit > 200 {
            return Err(crate::Error::InvalidRunsListQuery(
                "limit must be between 1 and 200".into(),
            ));
        }
        if offset < 0 {
            return Err(crate::Error::InvalidRunsListQuery(
                "offset must be non-negative".into(),
            ));
        }
        Ok(Self {
            limit,
            offset,
            trigger: trigger.filter(|t| !t.is_empty()),
            status: status.filter(|s| !s.is_empty()),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ListRunsResponse {
    pub runs: Vec<RunListItem>,
    pub total: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunListItem {
    pub id: i64,
    pub trigger: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_sec: Option<i64>,
    pub project_total: Option<i64>,
    pub project_skipped: i64,
}

pub async fn list_recent_runs(
    pool: &sqlx::SqlitePool,
    limit: i64,
) -> crate::Result<Vec<RunListItem>> {
    sqlx::query_as::<_, RunListItem>(
        "SELECT id, trigger, status, started_at, finished_at, duration_sec,
                project_total, project_skipped
         FROM runs
         ORDER BY started_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::Error::Database)
}

pub async fn list_runs(
    pool: &sqlx::SqlitePool,
    filter: &ListRunsFilter,
) -> crate::Result<ListRunsResponse> {
    let mut count_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT COUNT(*) FROM runs");
    push_run_filters(&mut count_qb, filter);
    let total: i64 = count_qb
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(crate::Error::Database)?;

    let mut list_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT id, trigger, status, started_at, finished_at, duration_sec,
                project_total, project_skipped
         FROM runs",
    );
    push_run_filters(&mut list_qb, filter);
    list_qb.push(" ORDER BY started_at DESC LIMIT ");
    list_qb.push_bind(filter.limit);
    list_qb.push(" OFFSET ");
    list_qb.push_bind(filter.offset);

    let runs = list_qb
        .build_query_as::<RunListItem>()
        .fetch_all(pool)
        .await
        .map_err(crate::Error::Database)?;

    Ok(ListRunsResponse { runs, total })
}

fn push_run_filters<'args>(
    qb: &mut sqlx::QueryBuilder<'args, sqlx::Sqlite>,
    filter: &'args ListRunsFilter,
) {
    let mut has_where = false;
    if let Some(trigger) = filter.trigger.as_ref() {
        qb.push(if has_where { " AND " } else { " WHERE " });
        qb.push("trigger = ");
        qb.push_bind(trigger);
        has_where = true;
    }
    if let Some(status) = filter.status.as_ref() {
        qb.push(if has_where { " AND " } else { " WHERE " });
        qb.push("status = ");
        qb.push_bind(status);
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunProjectStatusRow {
    pub project_id: i64,
    pub name: String,
    pub state: String,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_sec: Option<i64>,
}

pub async fn list_run_project_statuses(
    pool: &sqlx::SqlitePool,
    run_id: i64,
) -> crate::Result<Vec<RunProjectStatusRow>> {
    sqlx::query_as::<_, RunProjectStatusRow>(
        "SELECT rp.project_id, p.name, rp.state, rp.error,
                rp.started_at, rp.finished_at, rp.duration_sec
         FROM run_projects rp
         INNER JOIN projects p ON p.id = rp.project_id
         WHERE rp.run_id = ?
         ORDER BY rp.id",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(crate::Error::Database)
}

const SKIP_SUMMARY_ITEMS_CAP: usize = 100;

#[derive(Debug, Clone, Serialize)]
pub struct SkipSummaryItem {
    pub mr_iid: i64,
    pub skip_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkipSummary {
    pub by_reason: std::collections::BTreeMap<String, i64>,
    pub items: Vec<SkipSummaryItem>,
}

impl SkipSummary {
    pub fn empty() -> Self {
        Self {
            by_reason: std::collections::BTreeMap::new(),
            items: Vec::new(),
        }
    }
}

/// Build skip summary from `eligible_mrs.json`. Missing/unreadable → empty summary.
pub fn load_skip_summary(data_root: &Path, run_id: i64, project_id: i64) -> SkipSummary {
    let path = eligible_mrs_path(data_root, run_id, project_id);
    let Ok(file) = crate::mr_reviews::read_eligible_mrs(&path) else {
        return SkipSummary::empty();
    };

    let mut by_reason = std::collections::BTreeMap::new();
    for skipped in &file.skipped {
        *by_reason.entry(skipped.skip_reason.clone()).or_insert(0) += 1;
    }

    let items = file
        .skipped
        .into_iter()
        .take(SKIP_SUMMARY_ITEMS_CAP)
        .map(|skipped| SkipSummaryItem {
            mr_iid: skipped.mr_iid,
            skip_reason: skipped.skip_reason,
        })
        .collect();

    SkipSummary { by_reason, items }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, PartialEq, Eq)]
pub struct WeeklyReportPerson {
    pub person_id: i64,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MrDraftsOutput {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeeklyReportsOutput {
    pub people: Vec<WeeklyReportPerson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectOutputs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mr_drafts: Option<MrDraftsOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly_reports: Option<WeeklyReportsOutput>,
}

pub async fn list_weekly_report_people(
    pool: &sqlx::SqlitePool,
    run_id: i64,
    project_id: i64,
) -> crate::Result<Vec<WeeklyReportPerson>> {
    // DISTINCT: ingest replay can leave one person with several report rows
    // (distinct report_date) under a single run; the outputs hint lists people,
    // so each must appear once.
    sqlx::query_as::<_, WeeklyReportPerson>(
        "SELECT DISTINCT r.person_id, p.display_name
         FROM reports r
         INNER JOIN people p ON p.id = r.person_id
         WHERE r.run_id = ? AND r.project_id = ?
         ORDER BY p.display_name COLLATE NOCASE, r.person_id",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(crate::Error::Database)
}

/// Derive outputs for one project. Missing drafts dir → no mr_drafts; empty reports → no weekly.
pub async fn load_project_outputs(
    pool: &sqlx::SqlitePool,
    data_root: &Path,
    run_id: i64,
    project_id: i64,
) -> crate::Result<Option<ProjectOutputs>> {
    let draft_count = count_mr_draft_md_files(data_root, run_id, project_id);
    let people = list_weekly_report_people(pool, run_id, project_id).await?;
    let mr_drafts = if draft_count > 0 {
        Some(MrDraftsOutput { count: draft_count })
    } else {
        None
    };
    let weekly_reports = if people.is_empty() {
        None
    } else {
        Some(WeeklyReportsOutput { people })
    };
    if mr_drafts.is_none() && weekly_reports.is_none() {
        Ok(None)
    } else {
        Ok(Some(ProjectOutputs {
            mr_drafts,
            weekly_reports,
        }))
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ManifestOpenPending {
    pub id: i64,
    pub person_id: i64,
    /// Immutable on-disk directory segment and summary `person` key.
    pub folder_name: String,
    /// Current human-readable label; for report prose only, never a path.
    pub display_name: String,
    pub question: String,
}

#[derive(Serialize)]
pub struct RunManifest<'a> {
    pub mode: &'static str,
    pub project_name: &'a str,
    pub repo_path: &'a str,
    pub report_root: String,
    pub person_report_root: String,
    /// Project-level ADR directory: `{report_root}/.notes`.
    pub notes_dir: String,
    pub run_date: String,
    pub since: String,
    pub output_contract: &'static str,
    pub authors: Vec<ManifestAuthor>,
    /// Open pending questions for this project so the workflow can reuse exact wording.
    pub open_pending: Vec<ManifestOpenPending>,
    /// Observation snippets under `report_root` that may be folded into the weekly summary.
    /// Populated from `mr_reviews` rows with `status='published'`; draft/ignored snippets stay out.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub published_pending_snippets: Vec<String>,
    /// Global review ignore list. The weekly agent runs its own git commands,
    /// so the workflow instructs it to append these as exclude pathspecs.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ignore_globs: Vec<String>,
}

pub async fn load_open_pending_for_project(
    pool: &sqlx::SqlitePool,
    project_id: i64,
) -> crate::Result<Vec<ManifestOpenPending>> {
    sqlx::query_as::<_, ManifestOpenPending>(
        "SELECT pi.id, pi.person_id, p.folder_name, p.display_name, pi.question
         FROM pending_items pi
         INNER JOIN people p ON p.id = pi.person_id
         WHERE pi.project_id = ? AND pi.status = 'open'
         ORDER BY pi.person_id ASC, pi.id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(crate::Error::Database)
}

pub fn manifest_path(data_root: &Path, run_id: i64, project_id: i64) -> PathBuf {
    data_root
        .join("runs")
        .join(run_id.to_string())
        .join("projects")
        .join(project_id.to_string())
        .join("manifest.json")
}

/// `{DATA_ROOT}/reports/{project_name}/.notes` — project ADR store (index + adr-*.md).
pub fn project_notes_dir(data_root: &Path, project_name: &str) -> PathBuf {
    data_root
        .join("reports")
        .join(project_name)
        .join(".notes")
}

fn path_display_normalized(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Filter the three person-bearing manifest blocks down to a single person.
/// `authors` and `open_pending` match by `person_id`; snippet paths match by
/// leading path segment equal to `display_name` (snippet paths are produced as
/// `{person_folder}/_pending/...`). Pure so the scope contract is unit-testable
/// without git or a live manifest write.
fn retain_person_scope(
    authors: &mut Vec<ManifestAuthor>,
    open_pending: &mut Vec<ManifestOpenPending>,
    published_pending_snippets: &mut Vec<String>,
    person_id: i64,
    display_name: &str,
) {
    authors.retain(|a| a.person_id == person_id);
    open_pending.retain(|p| p.person_id == person_id);
    published_pending_snippets.retain(|snippet| {
        snippet
            .replace('\\', "/")
            .split('/')
            .next()
            .map(|segment| segment == display_name)
            .unwrap_or(false)
    });
}

pub async fn write_weekly_manifest(
    pool: &sqlx::SqlitePool,
    data_root: &Path,
    run_id: i64,
    project: &ProjectRow,
    repo_path: &str,
    person_id: Option<i64>,
) -> crate::Result<PathBuf> {
    let path = manifest_path(data_root, run_id, project.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let run_date = Utc::now().format("%Y-%m-%d").to_string();
    let since = (Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let report_root_path = data_root.join("reports").join(&project.name);
    let report_root = path_display_normalized(&report_root_path);
    let person_report_root = path_display_normalized(
        &data_root
            .join("reports")
            .join(crate::person_trends::PERSON_REPORT_DIR),
    );
    let notes_dir = path_display_normalized(&project_notes_dir(data_root, &project.name));

    let authors = identity::prepare_manifest_authors(
        pool,
        Path::new(repo_path),
        project.id,
        &since,
        &run_date,
    )
    .await?;

    let published_pending_snippets = crate::mr_reviews::load_published_pending_snippets(
        pool,
        project.id,
        &report_root_path,
    )
    .await?;
    let open_pending = load_open_pending_for_project(pool, project.id).await?;

    let (mut authors, mut open_pending, mut published_pending_snippets) =
        (authors, open_pending, published_pending_snippets);
    if let Some(pid) = person_id {
        let display_name: String =
            sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
                .bind(pid)
                .fetch_optional(pool)
                .await
                .map_err(crate::Error::Database)?
                .unwrap_or_default();
        retain_person_scope(
            &mut authors,
            &mut open_pending,
            &mut published_pending_snippets,
            pid,
            &display_name,
        );
    }

    let manifest = RunManifest {
        mode: "weekly_batch",
        project_name: &project.name,
        repo_path,
        report_root,
        person_report_root,
        notes_dir,
        run_date,
        since,
        output_contract: "output-contract.md",
        authors,
        open_pending,
        published_pending_snippets,
        ignore_globs: crate::review_settings::load(pool).await?.ignore_globs,
    };

    let json = serde_json::to_string_pretty(&manifest).map_err(|err| {
        crate::Error::SummaryParse(format!("manifest json: {err}"))
    })?;
    std::fs::write(&path, json)?;
    Ok(path)
}

pub fn parse_mr_review_skip_labels(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_else(|_| {
        DEFAULT_MR_REVIEW_SKIP_LABELS
            .iter()
            .map(|label| (*label).to_string())
            .collect()
    })
}

pub fn eligible_mrs_path(data_root: &Path, run_id: i64, project_id: i64) -> PathBuf {
    data_root
        .join("runs")
        .join(run_id.to_string())
        .join("projects")
        .join(project_id.to_string())
        .join("eligible_mrs.json")
}

pub fn mr_poll_draft_dir(data_root: &Path, run_id: i64, project_id: i64) -> PathBuf {
    data_root
        .join("runs")
        .join(run_id.to_string())
        .join("projects")
        .join(project_id.to_string())
        .join("drafts")
}

/// Count `*.md` files directly under the run project's drafts directory.
/// Missing or unreadable directories return 0 (must not fail run detail).
pub fn count_mr_draft_md_files(data_root: &Path, run_id: i64, project_id: i64) -> i64 {
    let dir = mr_poll_draft_dir(data_root, run_id, project_id);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut count = 0i64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            count += 1;
        }
    }
    count
}

/// Absolute paths to precomputed change materials for one MR agent subprocess.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestChangeMaterials {
    pub change_log_path: String,
    pub change_stat_path: String,
    pub change_diff_path: String,
}

impl ManifestChangeMaterials {
    pub fn from_paths(paths: &crate::mr_change_materials::ChangeMaterialPaths) -> Self {
        Self {
            change_log_path: paths
                .change_log_path
                .display()
                .to_string()
                .replace('\\', "/"),
            change_stat_path: paths
                .change_stat_path
                .display()
                .to_string()
                .replace('\\', "/"),
            change_diff_path: paths
                .change_diff_path
                .display()
                .to_string()
                .replace('\\', "/"),
        }
    }
}

#[derive(Serialize)]
pub struct MrPollManifest<'a> {
    pub mode: &'static str,
    pub project_name: &'a str,
    pub repo_path: &'a str,
    pub draft_dir: String,
    pub pending_dir: String,
    /// Project-level ADR directory: `reports/{project}/.notes`.
    pub notes_dir: String,
    /// Project-layer monthly file for this person (`reports/{project}/{person}/YYYY-MM.md`).
    /// Per-MR observation sessions are appended here in addition to `_pending` snippets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person_month_md_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    pub eligible_mrs_path: String,
    pub mr_review_skip_labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mr_review_require_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mr_iid: Option<i64>,
    /// Previously published AI reviews for this MR (oldest first). Used by round 2+.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub prior_published_reviews: Vec<PriorPublishedReview>,
    /// Precomputed `git log` / `diff --stat` / `diff` for the agent (per-MR only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_log_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_stat_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_diff_path: Option<String>,
    /// Global review ignore list. The precomputed diff already excludes these,
    /// but the agent has its own shell — the workflow instructs it to append
    /// the same exclusions to any git command it runs itself.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ignore_globs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PriorPublishedReview {
    pub review_round: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    pub body: String,
}

pub async fn write_mr_poll_manifest(
    data_root: &Path,
    run_id: i64,
    project: &MrPollProjectRow,
    repo_path: &str,
    reviewer_username: Option<&str>,
    mr_iid: Option<i64>,
    prior_published_reviews: Vec<PriorPublishedReview>,
    change_materials: Option<&ManifestChangeMaterials>,
    // MR author folder under reports/{project}/ (people.folder_name, the
    // immutable path key). When set, pending_dir becomes
    // reports/{project}/{folder_name}/_pending.
    observation_person: Option<&str>,
    ignore_globs: &[String],
) -> crate::Result<PathBuf> {
    let path = manifest_path(data_root, run_id, project.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let since = (Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let draft_dir = path_display_normalized(&mr_poll_draft_dir(data_root, run_id, project.id));
    let pending_dir = match observation_person.map(str::trim).filter(|s| !s.is_empty()) {
        Some(person) => {
            let dir = data_root
                .join("reports")
                .join(&project.name)
                .join(person)
                .join("_pending");
            std::fs::create_dir_all(&dir)?;
            path_display_normalized(&dir)
        }
        None => path_display_normalized(&data_root.join("reports").join(&project.name)),
    };
    let notes_dir = path_display_normalized(&project_notes_dir(data_root, &project.name));
    let person_month_md_path = observation_person.map(str::trim).filter(|s| !s.is_empty()).map(
        |person| {
            let month = Utc::now().format("%Y-%m").to_string();
            let path = data_root
                .join("reports")
                .join(&project.name)
                .join(person)
                .join(format!("{month}.md"));
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            path_display_normalized(&path)
        },
    );
    let eligible_path = path_display_normalized(&eligible_mrs_path(data_root, run_id, project.id));

    let manifest = MrPollManifest {
        mode: "mr_poll",
        project_name: &project.name,
        repo_path,
        draft_dir,
        pending_dir,
        notes_dir,
        person_month_md_path,
        reviewer_username: reviewer_username.map(str::to_string),
        since: Some(since),
        eligible_mrs_path: eligible_path,
        mr_review_skip_labels: parse_mr_review_skip_labels(&project.mr_review_skip_labels),
        mr_review_require_label: project.mr_review_require_label.clone(),
        mr_iid,
        prior_published_reviews,
        change_log_path: change_materials.map(|m| m.change_log_path.clone()),
        change_stat_path: change_materials.map(|m| m.change_stat_path.clone()),
        change_diff_path: change_materials.map(|m| m.change_diff_path.clone()),
        ignore_globs: ignore_globs.to_vec(),
    };

    let json = serde_json::to_string_pretty(&manifest).map_err(|err| {
        crate::Error::SummaryParse(format!("manifest json: {err}"))
    })?;
    std::fs::write(&path, json)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mr_scan_force_accepts_truthy_values() {
        assert!(parse_mr_scan_force(Some("1")));
        assert!(parse_mr_scan_force(Some("true")));
        assert!(parse_mr_scan_force(Some("TRUE")));
        assert!(!parse_mr_scan_force(None));
        assert!(!parse_mr_scan_force(Some("0")));
        assert!(!parse_mr_scan_force(Some("false")));
    }

    #[test]
    fn is_mr_trigger_matches_poll_triggers() {
        assert!(is_mr_trigger("mr_poll"));
        assert!(is_mr_trigger("manual_mr_poll"));
        assert!(!is_mr_trigger("manual_all"));
        assert!(!is_mr_trigger("schedule"));
    }

    #[test]
    fn parse_mr_review_skip_labels_uses_defaults_on_invalid_json() {
        let labels = parse_mr_review_skip_labels("not-json");
        assert_eq!(
            labels,
            vec![
                "wip".to_string(),
                "do-not-review".to_string(),
                "no-ai-review".to_string(),
            ]
        );
    }

    #[test]
    fn eligible_mrs_path_follows_run_layout() {
        let root = Path::new("/data/reviewer");
        let path = eligible_mrs_path(root, 9, 3);
        assert_eq!(
            path,
            Path::new("/data/reviewer/runs/9/projects/3/eligible_mrs.json")
        );
    }

    #[test]
    fn count_mr_draft_md_files_missing_dir_is_zero() {
        let temp = tempfile::tempdir().expect("tempdir");
        assert_eq!(count_mr_draft_md_files(temp.path(), 9, 3), 0);
    }

    #[test]
    fn count_mr_draft_md_files_counts_markdown_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dir = mr_poll_draft_dir(temp.path(), 9, 3);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("a.md"), "# a").expect("write a");
        std::fs::write(dir.join("b.md"), "# b").expect("write b");
        std::fs::write(dir.join("notes.txt"), "x").expect("write txt");
        std::fs::create_dir_all(dir.join("sub")).expect("subdir");
        assert_eq!(count_mr_draft_md_files(temp.path(), 9, 3), 2);
    }

    #[test]
    fn write_mr_poll_manifest_carries_ignore_globs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = MrPollProjectRow {
            id: 4,
            name: "beta".into(),
            repo_path: "/repos/beta".into(),
            mr_review_skip_labels: "[]".into(),
            mr_review_require_label: None,
        };
        let globs = vec!["*.lock".to_string(), "vendor/**".to_string()];

        let path = tokio::runtime::Runtime::new()
            .expect("rt")
            .block_on(write_mr_poll_manifest(
                temp.path(),
                11,
                &project,
                "/repos/beta/feat",
                None,
                Some(7),
                Vec::new(),
                None,
                None,
                &globs,
            ))
            .expect("write manifest");

        let raw = std::fs::read_to_string(&path).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(json["ignore_globs"][0], "*.lock");
        assert_eq!(json["ignore_globs"][1], "vendor/**");
    }

    #[test]
    fn write_mr_poll_manifest_includes_prior_published_reviews() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = MrPollProjectRow {
            id: 3,
            name: "alpha".into(),
            repo_path: "/repos/alpha".into(),
            mr_review_skip_labels: "[]".into(),
            mr_review_require_label: None,
        };
        let prior = vec![PriorPublishedReview {
            review_round: 1,
            published_at: Some("2026-07-12 10:00:00".into()),
            body: "Prior AI review body".into(),
        }];

        let materials = ManifestChangeMaterials {
            change_log_path: "/data/mr-68/change_log.txt".into(),
            change_stat_path: "/data/mr-68/change_stat.txt".into(),
            change_diff_path: "/data/mr-68/change.diff".into(),
        };
        let path = tokio::runtime::Runtime::new()
            .expect("rt")
            .block_on(write_mr_poll_manifest(
                temp.path(),
                9,
                &project,
                "/repos/alpha/feat",
                None,
                Some(68),
                prior,
                Some(&materials),
                Some("Alice Chen"),
                &[],
            ))
            .expect("write manifest");

        let raw = std::fs::read_to_string(&path).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(json["mr_iid"], 68);
        assert_eq!(json["prior_published_reviews"][0]["review_round"], 1);
        assert_eq!(
            json["prior_published_reviews"][0]["body"],
            "Prior AI review body"
        );
        assert_eq!(json["change_log_path"], "/data/mr-68/change_log.txt");
        assert_eq!(json["change_stat_path"], "/data/mr-68/change_stat.txt");
        assert_eq!(json["change_diff_path"], "/data/mr-68/change.diff");
        assert!(
            json.get("ignore_globs").is_none(),
            "empty ignore list must be omitted from the manifest"
        );
        let pending = json["pending_dir"].as_str().expect("pending_dir");
        assert!(
            pending.ends_with("reports/alpha/Alice Chen/_pending")
                || pending.ends_with(r"reports\alpha\Alice Chen\_pending"),
            "pending_dir should be reports/{{project}}/{{person}}/_pending, got {pending}"
        );
        assert!(
            temp.path()
                .join("reports")
                .join("alpha")
                .join("Alice Chen")
                .join("_pending")
                .is_dir(),
            "_pending directory should be created"
        );
        let month_path = json["person_month_md_path"].as_str().expect("month path");
        assert!(
            month_path.contains("reports/alpha/Alice Chen/")
                || month_path.contains(r"reports\alpha\Alice Chen\"),
            "person_month_md_path should be under person folder, got {month_path}"
        );
        assert!(
            month_path.ends_with(".md"),
            "person_month_md_path should be a .md file, got {month_path}"
        );
        let notes_dir = json["notes_dir"].as_str().expect("notes_dir");
        assert!(
            notes_dir.ends_with("reports/alpha/.notes")
                || notes_dir.ends_with(r"reports\alpha\.notes"),
            "notes_dir should be reports/{{project}}/.notes, got {notes_dir}"
        );
    }

    #[test]
    fn project_notes_dir_joins_reports_project_dot_notes() {
        let root = Path::new("/data/reviewer");
        let path = project_notes_dir(root, "game-backend");
        assert_eq!(
            path,
            Path::new("/data/reviewer/reports/game-backend/.notes")
        );
    }

    #[tokio::test]
    async fn list_weekly_report_people_empty_when_no_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pool = crate::db::init_pool(temp.path()).await.expect("init pool");
        sqlx::query("INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', '/r', 0)")
            .execute(&pool)
            .await
            .expect("insert project");
        let run_id = sqlx::query(
            "INSERT INTO runs (trigger, status, project_total) VALUES ('schedule', 'success', 1)",
        )
        .execute(&pool)
        .await
        .expect("insert run")
        .last_insert_rowid();
        let people = list_weekly_report_people(&pool, run_id, 1)
            .await
            .expect("list");
        assert!(people.is_empty());
    }

    #[tokio::test]
    async fn list_weekly_report_people_returns_person_id_and_display_name() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pool = crate::db::init_pool(temp.path()).await.expect("init pool");
        sqlx::query("INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', '/r', 0)")
            .execute(&pool)
            .await
            .expect("insert project");
        let person_id = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice Chen')")
            .execute(&pool)
            .await
            .expect("insert person")
            .last_insert_rowid();
        let run_id = sqlx::query(
            "INSERT INTO runs (trigger, status, project_total) VALUES ('schedule', 'success', 1)",
        )
        .execute(&pool)
        .await
        .expect("insert run")
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO reports (project_id, person_id, run_id, report_date, report_md_path, summary_md_path)
             VALUES (1, ?, ?, '2026-07-05', 'r.md', 's.md')",
        )
        .bind(person_id)
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert report");

        let people = list_weekly_report_people(&pool, run_id, 1)
            .await
            .expect("list");
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].person_id, person_id);
        assert_eq!(people[0].display_name, "Alice Chen");
    }

    #[tokio::test]
    async fn list_weekly_report_people_deduplicates_person_across_report_dates() {
        // Ingest replay re-stamps historical reports with the current run_id, so
        // one person can own several report rows (distinct report_date) under one
        // run. The outputs hint lists people, so each must appear exactly once.
        let temp = tempfile::tempdir().expect("tempdir");
        let pool = crate::db::init_pool(temp.path()).await.expect("init pool");
        sqlx::query("INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', '/r', 0)")
            .execute(&pool)
            .await
            .expect("insert project");
        let person_id = sqlx::query("INSERT INTO people (display_name) VALUES ('Power')")
            .execute(&pool)
            .await
            .expect("insert person")
            .last_insert_rowid();
        let run_id = sqlx::query(
            "INSERT INTO runs (trigger, status, project_total) VALUES ('schedule', 'success', 1)",
        )
        .execute(&pool)
        .await
        .expect("insert run")
        .last_insert_rowid();
        for date in ["2026-07-12", "2026-07-20", "2026-07-21"] {
            sqlx::query(
                "INSERT INTO reports (project_id, person_id, run_id, report_date, report_md_path, summary_md_path)
                 VALUES (1, ?, ?, ?, 'r.md', 's.md')",
            )
            .bind(person_id)
            .bind(run_id)
            .bind(date)
            .execute(&pool)
            .await
            .expect("insert report");
        }

        let people = list_weekly_report_people(&pool, run_id, 1)
            .await
            .expect("list");
        assert_eq!(people.len(), 1, "one person must not repeat per report_date");
        assert_eq!(people[0].person_id, person_id);
    }

    fn author(person_id: i64, display_name: &str) -> ManifestAuthor {
        ManifestAuthor {
            email: format!("{display_name}@example.com"),
            git_name: display_name.to_string(),
            person_id,
            folder_name: display_name.to_string(),
            display_name: display_name.to_string(),
        }
    }

    fn open_pending(id: i64, person_id: i64, display_name: &str) -> ManifestOpenPending {
        ManifestOpenPending {
            id,
            person_id,
            folder_name: display_name.to_string(),
            display_name: display_name.to_string(),
            question: format!("q{id}"),
        }
    }

    #[test]
    fn retain_person_scope_keeps_only_target_person() {
        let mut authors = vec![author(1, "Alice Chen"), author(2, "Bob")];
        let mut open = vec![
            open_pending(10, 1, "Alice Chen"),
            open_pending(11, 2, "Bob"),
        ];
        let mut snippets = vec![
            "Alice Chen/_pending/mr-1-round-1.md".to_string(),
            "Bob/_pending/mr-2-round-1.md".to_string(),
        ];

        retain_person_scope(&mut authors, &mut open, &mut snippets, 1, "Alice Chen");

        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].person_id, 1);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].person_id, 1);
        assert_eq!(snippets, vec!["Alice Chen/_pending/mr-1-round-1.md".to_string()]);
    }

    #[test]
    fn retain_person_scope_no_match_yields_empty() {
        let mut authors = vec![author(1, "Alice Chen"), author(2, "Bob")];
        let mut open = vec![open_pending(10, 1, "Alice Chen")];
        let mut snippets = vec!["Alice Chen/_pending/mr-1-round-1.md".to_string()];

        // person 3 has no window activity: every block filters to empty, no panic.
        retain_person_scope(&mut authors, &mut open, &mut snippets, 3, "Carol");

        assert!(authors.is_empty());
        assert!(open.is_empty());
        assert!(snippets.is_empty());
    }
}
