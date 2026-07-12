use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use sqlx::Row;

use crate::identity::{self, ManifestAuthor};

pub fn is_mr_trigger(trigger: &str) -> bool {
    matches!(trigger, "mr_poll" | "manual_mr_poll")
}

pub const DEFAULT_MR_REVIEW_SKIP_LABELS: &[&str] =
    &["wip", "do-not-review", "no-ai-review"];

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
    let row = sqlx::query_as::<_, RunProjectRow>(
        "SELECT rp.id, rp.run_id, rp.project_id, p.name, p.repo_path, r.trigger, r.mr_scan_force
         FROM run_projects rp
         INNER JOIN projects p ON p.id = rp.project_id
         INNER JOIN runs r ON r.id = rp.run_id
         WHERE r.status = 'running' AND rp.state = 'queued'
         ORDER BY rp.id
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)?;
    Ok(row)
}

pub async fn mark_run_project_running(
    pool: &sqlx::SqlitePool,
    run_project_id: i64,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE run_projects
         SET state = 'running', started_at = datetime('now')
         WHERE id = ?",
    )
    .bind(run_project_id)
    .execute(pool)
    .await
    .map_err(crate::Error::Database)?;
    Ok(())
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

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ManifestOpenPending {
    pub id: i64,
    pub person_id: i64,
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
}

pub async fn load_open_pending_for_project(
    pool: &sqlx::SqlitePool,
    project_id: i64,
) -> crate::Result<Vec<ManifestOpenPending>> {
    sqlx::query_as::<_, ManifestOpenPending>(
        "SELECT pi.id, pi.person_id, p.display_name, pi.question
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

pub async fn write_weekly_manifest(
    pool: &sqlx::SqlitePool,
    data_root: &Path,
    run_id: i64,
    project: &ProjectRow,
    repo_path: &str,
) -> crate::Result<PathBuf> {
    let path = manifest_path(data_root, run_id, project.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let run_date = Utc::now().format("%Y-%m-%d").to_string();
    let since = (Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let report_root = data_root
        .join("reports")
        .join(&project.name)
        .display()
        .to_string()
        .replace('\\', "/");
    let person_report_root = data_root
        .join("reports")
        .join(crate::person_trends::PERSON_REPORT_DIR)
        .display()
        .to_string()
        .replace('\\', "/");

    let authors = identity::prepare_manifest_authors(
        pool,
        Path::new(repo_path),
        project.id,
        &since,
        &run_date,
    )
    .await?;

    let published_pending_snippets =
        crate::mr_reviews::load_published_pending_snippets(pool, project.id).await?;
    let open_pending = load_open_pending_for_project(pool, project.id).await?;

    let manifest = RunManifest {
        mode: "weekly_batch",
        project_name: &project.name,
        repo_path,
        report_root,
        person_report_root,
        run_date,
        since,
        output_contract: "output-contract.md",
        authors,
        open_pending,
        published_pending_snippets,
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

#[derive(Serialize)]
pub struct MrPollManifest<'a> {
    pub mode: &'static str,
    pub project_name: &'a str,
    pub repo_path: &'a str,
    pub draft_dir: String,
    pub pending_dir: String,
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
) -> crate::Result<PathBuf> {
    let path = manifest_path(data_root, run_id, project.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let since = (Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let draft_dir = mr_poll_draft_dir(data_root, run_id, project.id)
        .display()
        .to_string()
        .replace('\\', "/");
    let pending_dir = data_root
        .join("reports")
        .join(&project.name)
        .display()
        .to_string()
        .replace('\\', "/");
    let eligible_path = eligible_mrs_path(data_root, run_id, project.id)
        .display()
        .to_string()
        .replace('\\', "/");

    let manifest = MrPollManifest {
        mode: "mr_poll",
        project_name: &project.name,
        repo_path,
        draft_dir,
        pending_dir,
        reviewer_username: reviewer_username.map(str::to_string),
        since: Some(since),
        eligible_mrs_path: eligible_path,
        mr_review_skip_labels: parse_mr_review_skip_labels(&project.mr_review_skip_labels),
        mr_review_require_label: project.mr_review_require_label.clone(),
        mr_iid,
        prior_published_reviews,
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
    }
}
