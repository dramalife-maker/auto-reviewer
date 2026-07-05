use std::path::Path;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::summary::SummarySections;
use crate::Error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PersonListItem {
    pub id: i64,
    pub display_name: String,
    pub project_count: i64,
    pub unread_count: i64,
    pub open_pending_count: i64,
}

#[derive(Debug, Serialize)]
pub struct LatestReportItem {
    pub project_name: String,
    pub one_line: Option<String>,
    pub mr_count: Option<i64>,
    pub commit_count: Option<i64>,
    pub highlights: Vec<String>,
    pub growth: Vec<String>,
    pub pending: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LatestReportsResponse {
    pub report_date: String,
    pub projects: Vec<LatestReportItem>,
}

pub async fn list_people(pool: &SqlitePool) -> Result<Vec<PersonListItem>, Error> {
    sqlx::query_as::<_, PersonListItem>(
        "SELECT p.id, p.display_name,
                COUNT(DISTINCT r.project_id) AS project_count,
                COALESCE(SUM(CASE WHEN r.is_read = 0 THEN 1 ELSE 0 END), 0) AS unread_count,
                (SELECT COUNT(*) FROM pending_items pi
                   WHERE pi.person_id = p.id AND pi.status = 'open') AS open_pending_count
         FROM people p
         LEFT JOIN reports r ON r.person_id = p.id
         GROUP BY p.id, p.display_name
         ORDER BY p.display_name",
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

pub async fn latest_reports_for_person(
    pool: &SqlitePool,
    person_id: i64,
) -> Result<Option<LatestReportsResponse>, Error> {
    let report_date: Option<String> =
        sqlx::query_scalar("SELECT MAX(report_date) FROM reports WHERE person_id = ?")
            .bind(person_id)
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

    let Some(report_date) = report_date else {
        return Ok(None);
    };

    let rows = sqlx::query_as::<_, ReportSummaryRow>(
        "SELECT pr.name AS project_name, r.one_line, r.mr_count, r.commit_count, r.summary_md_path
         FROM reports r
         INNER JOIN projects pr ON pr.id = r.project_id
         WHERE r.person_id = ? AND r.report_date = ?
         ORDER BY pr.name",
    )
    .bind(person_id)
    .bind(&report_date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut projects = Vec::new();
    for row in rows {
        let sections = SummarySections::from_summary_file(Path::new(&row.summary_md_path))?;
        projects.push(LatestReportItem {
            project_name: row.project_name,
            one_line: row.one_line,
            mr_count: row.mr_count,
            commit_count: row.commit_count,
            highlights: sections.highlights,
            growth: sections.growth,
            pending: sections.pending,
        });
    }

    Ok(Some(LatestReportsResponse {
        report_date,
        projects,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct ReportSummaryRow {
    project_name: String,
    one_line: Option<String>,
    mr_count: Option<i64>,
    commit_count: Option<i64>,
    summary_md_path: String,
}

pub async fn mark_report_read(pool: &SqlitePool, report_id: i64) -> Result<bool, Error> {
    let result = sqlx::query("UPDATE reports SET is_read = 1 WHERE id = ?")
        .bind(report_id)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(result.rows_affected() > 0)
}

pub async fn unread_count_for_person(pool: &SqlitePool, person_id: i64) -> Result<i64, Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reports WHERE person_id = ? AND is_read = 0",
    )
    .bind(person_id)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;
    Ok(count)
}
