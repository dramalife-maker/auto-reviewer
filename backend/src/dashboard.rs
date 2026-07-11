use serde::Serialize;
use sqlx::SqlitePool;

use crate::runs::{self, RunListItem};
use crate::schedule::{compute_next_run_at, format_mr_poll_label, format_schedule_label, load_schedule_config};
use crate::Error;

#[derive(Debug, Serialize)]
pub struct DashboardLastRun {
    pub started_at: String,
    pub duration_sec: Option<i64>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub project_count: i64,
    pub person_count: i64,
    pub unread_count: i64,
    pub pending_count: i64,
    pub mr_draft_count: i64,
}

#[derive(Debug, Serialize)]
pub struct DashboardRecentReport {
    pub report_id: i64,
    pub person_id: i64,
    pub person_name: String,
    pub project_name: String,
    pub is_read: bool,
    pub pending_count: i64,
}

#[derive(Debug, Serialize)]
pub struct DashboardSchedule {
    pub label: String,
    pub next_run_at: Option<String>,
    pub enabled: bool,
    pub mr_poll_interval_min: i64,
    pub mr_poll_label: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub last_run: Option<DashboardLastRun>,
    pub stats: DashboardStats,
    pub recent_reports: Vec<DashboardRecentReport>,
    pub recent_runs: Vec<RunListItem>,
    pub schedule: DashboardSchedule,
}

#[derive(Debug, sqlx::FromRow)]
struct LastRunRow {
    started_at: String,
    duration_sec: Option<i64>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct RecentReportRow {
    report_id: i64,
    person_id: i64,
    person_name: String,
    project_name: String,
    is_read: i64,
    pending_count: i64,
}

pub async fn load_dashboard(pool: &SqlitePool) -> Result<DashboardResponse, Error> {
    let last_run = sqlx::query_as::<_, LastRunRow>(
        "SELECT started_at, duration_sec, status
         FROM runs
         WHERE finished_at IS NOT NULL
         ORDER BY finished_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?
    .map(|row| DashboardLastRun {
        started_at: row.started_at,
        duration_sec: row.duration_sec,
        status: row.status,
    });

    let stats = DashboardStats {
        project_count: count_scalar(pool, "SELECT COUNT(*) FROM projects").await?,
        person_count: count_scalar(pool, "SELECT COUNT(*) FROM people").await?,
        unread_count: count_scalar(
            pool,
            "SELECT COUNT(*) FROM reports WHERE is_read = 0",
        )
        .await?,
        pending_count: count_scalar(
            pool,
            "SELECT COUNT(*) FROM pending_items WHERE status = 'open'",
        )
        .await?,
        mr_draft_count: count_scalar(
            pool,
            "SELECT COUNT(*) FROM mr_reviews WHERE status = 'draft'",
        )
        .await?,
    };

    let recent_rows = sqlx::query_as::<_, RecentReportRow>(
        "SELECT r.id AS report_id,
                p.id AS person_id,
                p.display_name AS person_name,
                pr.name AS project_name,
                r.is_read,
                COALESCE((
                    SELECT COUNT(*)
                    FROM pending_items pi
                    WHERE pi.person_id = r.person_id
                      AND pi.project_id = r.project_id
                      AND pi.status = 'open'
                ), 0) AS pending_count
         FROM reports r
         INNER JOIN people p ON p.id = r.person_id
         INNER JOIN projects pr ON pr.id = r.project_id
         WHERE r.report_date = (SELECT MAX(report_date) FROM reports)
         ORDER BY r.is_read ASC, p.display_name, pr.name
         LIMIT 20",
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let recent_reports = recent_rows
        .into_iter()
        .map(|row| DashboardRecentReport {
            report_id: row.report_id,
            person_id: row.person_id,
            person_name: row.person_name,
            project_name: row.project_name,
            is_read: row.is_read != 0,
            pending_count: row.pending_count,
        })
        .collect();

    let schedule_config = load_schedule_config(pool).await?;
    let schedule = DashboardSchedule {
        label: format_schedule_label(&schedule_config),
        next_run_at: compute_next_run_at(&schedule_config)?,
        enabled: schedule_config.enabled != 0,
        mr_poll_interval_min: schedule_config.mr_poll_interval_min,
        mr_poll_label: format_mr_poll_label(schedule_config.mr_poll_interval_min),
    };

    let recent_runs = runs::list_recent_runs(pool, 5).await?;

    Ok(DashboardResponse {
        last_run,
        stats,
        recent_reports,
        recent_runs,
        schedule,
    })
}

async fn count_scalar(pool: &SqlitePool, query: &str) -> Result<i64, Error> {
    let row = sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(row)
}
