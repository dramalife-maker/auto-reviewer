use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use sqlx::Row;

use crate::identity::{self, ManifestAuthor};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: i64,
    pub name: String,
    pub repo_path: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RunProjectRow {
    pub id: i64,
    pub run_id: i64,
    pub project_id: i64,
    pub name: String,
    pub repo_path: String,
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

pub async fn create_manual_all_run(pool: &sqlx::SqlitePool) -> crate::Result<i64> {
    create_batch_run(pool, "manual_all").await
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
        "SELECT rp.id, rp.run_id, rp.project_id, p.name, p.repo_path
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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RunRow {
    pub id: i64,
    pub trigger: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub project_total: Option<i64>,
    pub project_skipped: i64,
}

pub async fn get_run(pool: &sqlx::SqlitePool, run_id: i64) -> crate::Result<Option<RunRow>> {
    sqlx::query_as::<_, RunRow>(
        "SELECT id, trigger, status, started_at, finished_at, project_total, project_skipped
         FROM runs WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::Error::Database)
}

#[derive(Serialize)]
pub struct RunManifest<'a> {
    pub mode: &'static str,
    pub project_name: &'a str,
    pub repo_path: &'a str,
    pub report_root: String,
    pub run_date: String,
    pub since: String,
    pub output_contract: &'static str,
    pub authors: Vec<ManifestAuthor>,
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

    let authors = identity::prepare_manifest_authors(
        pool,
        Path::new(repo_path),
        project.id,
        &since,
        &run_date,
    )
    .await?;

    let manifest = RunManifest {
        mode: "weekly_batch",
        project_name: &project.name,
        repo_path,
        report_root,
        run_date,
        since,
        output_contract: "output-contract.md",
        authors,
    };

    let json = serde_json::to_string_pretty(&manifest).map_err(|err| {
        crate::Error::SummaryParse(format!("manifest json: {err}"))
    })?;
    std::fs::write(&path, json)?;
    Ok(path)
}
