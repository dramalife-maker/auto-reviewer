use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::SqlitePool;
use tracing::warn;

use crate::pending_items::PendingItem;
use crate::summary::SummarySections;
use crate::Error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PersonListItem {
    pub id: i64,
    pub display_name: String,
    pub project_count: i64,
    pub unread_count: i64,
    pub open_pending_count: i64,
    pub identity_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingObservation {
    pub mr_iid: i64,
    pub review_round: i64,
    pub mr_title: Option<String>,
    pub status: String,
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct LatestReportItem {
    pub id: i64,
    pub is_read: bool,
    pub project_name: String,
    pub one_line: Option<String>,
    pub mr_count: Option<i64>,
    pub commit_count: Option<i64>,
    pub highlights: Vec<String>,
    pub growth: Vec<String>,
    pub pending_items: Vec<PendingItem>,
    pub pending_observations: Vec<PendingObservation>,
}

#[derive(Debug, Serialize)]
pub struct LatestReportsResponse {
    pub report_date: Option<String>,
    pub projects: Vec<LatestReportItem>,
}

pub async fn list_people(pool: &SqlitePool) -> Result<Vec<PersonListItem>, Error> {
    sqlx::query_as::<_, PersonListItem>(
        "SELECT p.id, p.display_name,
                COUNT(DISTINCT r.project_id) AS project_count,
                COALESCE(SUM(CASE WHEN r.is_read = 0 THEN 1 ELSE 0 END), 0) AS unread_count,
                (SELECT COUNT(*) FROM pending_items pi
                   WHERE pi.person_id = p.id AND pi.status = 'open') AS open_pending_count,
                (SELECT COUNT(*) FROM person_identities pi2
                   WHERE pi2.person_id = p.id) AS identity_count
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
    data_root: &Path,
    person_id: i64,
) -> Result<Option<LatestReportsResponse>, Error> {
    let display_name: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
            .bind(person_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

    let Some(display_name) = display_name else {
        return Ok(None);
    };

    let report_date: Option<String> =
        sqlx::query_scalar("SELECT MAX(report_date) FROM reports WHERE person_id = ?")
            .bind(person_id)
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

    let open_pending_items = sqlx::query_as::<_, PendingItem>(
        "SELECT pi.id, pi.person_id, pi.project_id, pr.name AS project_name,
                pi.report_id, pi.question, pi.status, pi.raised_date,
                pi.resolved_date, pi.resolution_note
         FROM pending_items pi
         INNER JOIN projects pr ON pr.id = pi.project_id
         WHERE pi.person_id = ? AND pi.status = 'open'
         ORDER BY pi.project_id, pi.raised_date DESC, pi.id DESC",
    )
    .bind(person_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut pending_by_project: HashMap<i64, Vec<PendingItem>> = HashMap::new();
    for item in open_pending_items {
        pending_by_project
            .entry(item.project_id)
            .or_default()
            .push(item);
    }

    let mut projects = Vec::new();
    let mut covered_project_ids = std::collections::HashSet::new();

    if let Some(ref report_date) = report_date {
        let rows = sqlx::query_as::<_, ReportSummaryRow>(
            "SELECT r.id, r.is_read, pr.id AS project_id, pr.name AS project_name, r.one_line, r.mr_count, r.commit_count, r.summary_md_path
             FROM reports r
             INNER JOIN projects pr ON pr.id = r.project_id
             WHERE r.person_id = ? AND r.report_date = ?
             ORDER BY pr.name",
        )
        .bind(person_id)
        .bind(report_date)
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?;

        for row in rows {
            covered_project_ids.insert(row.project_id);
            let sections = match SummarySections::from_summary_file(Path::new(&row.summary_md_path))
            {
                Ok(sections) => sections,
                Err(err) => {
                    warn!(
                        report_id = row.id,
                        path = %row.summary_md_path,
                        error = %err,
                        "skipping unreadable summary.md; serving report metadata without highlights/growth"
                    );
                    SummarySections {
                        highlights: Vec::new(),
                        growth: Vec::new(),
                    }
                }
            };
            let pending_items = pending_by_project
                .remove(&row.project_id)
                .unwrap_or_default();
            let pending_observations = load_pending_observations_for_project(
                pool,
                data_root,
                row.project_id,
                &row.project_name,
                &display_name,
            )
            .await?;

            projects.push(LatestReportItem {
                id: row.id,
                is_read: row.is_read != 0,
                project_name: row.project_name,
                one_line: row.one_line,
                mr_count: row.mr_count,
                commit_count: row.commit_count,
                highlights: sections.highlights,
                growth: sections.growth,
                pending_items,
                pending_observations,
            });
        }
    }

    let mut synthetic_candidates: HashMap<i64, String> = HashMap::new();
    for (project_id, items) in &pending_by_project {
        if covered_project_ids.contains(project_id) {
            continue;
        }
        if let Some(first) = items.first() {
            synthetic_candidates.insert(*project_id, first.project_name.clone());
        }
    }
    for (project_id, project_name) in
        discover_pending_observation_projects(pool, data_root, &display_name).await?
    {
        if covered_project_ids.contains(&project_id) {
            continue;
        }
        synthetic_candidates
            .entry(project_id)
            .or_insert(project_name);
    }

    for (project_id, project_name) in synthetic_candidates {
        let pending_items = pending_by_project
            .remove(&project_id)
            .unwrap_or_default();
        let pending_observations = load_pending_observations_for_project(
            pool,
            data_root,
            project_id,
            &project_name,
            &display_name,
        )
        .await?;
        if pending_items.is_empty() && pending_observations.is_empty() {
            continue;
        }
        projects.push(LatestReportItem {
            id: -project_id,
            is_read: true,
            project_name,
            one_line: None,
            mr_count: None,
            commit_count: None,
            highlights: Vec::new(),
            growth: Vec::new(),
            pending_items,
            pending_observations,
        });
    }

    projects.sort_by(|a, b| a.project_name.cmp(&b.project_name));

    Ok(Some(LatestReportsResponse {
        report_date,
        projects,
    }))
}

/// Find projects under `reports/` that have parseable `_pending/` snippets for this person.
async fn discover_pending_observation_projects(
    pool: &SqlitePool,
    data_root: &Path,
    person_display_name: &str,
) -> Result<Vec<(i64, String)>, Error> {
    let reports_root = data_root.join("reports");
    let Ok(entries) = std::fs::read_dir(&reports_root) else {
        return Ok(Vec::new());
    };

    let mut found = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(project_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if crate::person_trends::is_person_level_report_name(project_name) {
            continue;
        }
        let pending = pending_dir(data_root, project_name, person_display_name);
        if !pending_dir_has_parseable_snippet(&pending) {
            continue;
        }
        let project_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM projects WHERE name = ?")
                .bind(project_name)
                .fetch_optional(pool)
                .await
                .map_err(Error::Database)?;
        let Some(project_id) = project_id else {
            warn!(
                project = project_name,
                "skipping pending observations for unknown project folder"
            );
            continue;
        };
        found.push((project_id, project_name.to_string()));
    }
    Ok(found)
}

fn pending_dir_has_parseable_snippet(pending_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(pending_dir) else {
        return false;
    };
    entries.filter_map(|entry| entry.ok()).any(|entry| {
        let path = entry.path();
        path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(parse_pending_snippet_filename)
                .is_some()
    })
}

#[derive(Debug, sqlx::FromRow)]
struct ReportSummaryRow {
    id: i64,
    is_read: i64,
    project_id: i64,
    project_name: String,
    one_line: Option<String>,
    mr_count: Option<i64>,
    commit_count: Option<i64>,
    summary_md_path: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MrReviewLookupRow {
    mr_iid: i64,
    review_round: i64,
    mr_title: Option<String>,
    status: String,
}

/// Scan `reports/<project>/<person>/_pending/` and join `mr_reviews` for status/title.
async fn load_pending_observations_for_project(
    pool: &SqlitePool,
    data_root: &Path,
    project_id: i64,
    project_name: &str,
    person_display_name: &str,
) -> Result<Vec<PendingObservation>, Error> {
    let pending_dir = pending_dir(data_root, project_name, person_display_name);
    let Ok(entries) = std::fs::read_dir(&pending_dir) else {
        return Ok(Vec::new());
    };

    let review_rows = sqlx::query_as::<_, MrReviewLookupRow>(
        "SELECT mr_iid, review_round, mr_title, status
         FROM mr_reviews
         WHERE project_id = ?",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut review_by_key: HashMap<(i64, i64), (Option<String>, String)> = HashMap::new();
    for row in review_rows {
        review_by_key.insert(
            (row.mr_iid, row.review_round),
            (row.mr_title, row.status),
        );
    }

    let mut observations = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((mr_iid, review_round)) = parse_pending_snippet_filename(filename) else {
            warn!(
                path = %path.display(),
                "skipping pending observation with unparseable filename"
            );
            continue;
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) => {
                warn!(
                    path = %path.display(),
                    error = %err,
                    "skipping pending observation that failed to read"
                );
                continue;
            }
        };

        let (mr_title, status) = review_by_key
            .get(&(mr_iid, review_round))
            .map(|(title, status)| (title.clone(), status.clone()))
            .unwrap_or_else(|| (None, "unknown".to_string()));

        observations.push(PendingObservation {
            mr_iid,
            review_round,
            mr_title,
            status,
            filename: filename.to_string(),
            content,
        });
    }

    observations.sort_by(|a, b| {
        observation_status_rank(&a.status)
            .cmp(&observation_status_rank(&b.status))
            .then(a.mr_iid.cmp(&b.mr_iid))
            .then(a.review_round.cmp(&b.review_round))
    });

    Ok(observations)
}

fn pending_dir(data_root: &Path, project_name: &str, person_display_name: &str) -> PathBuf {
    data_root
        .join("reports")
        .join(project_name)
        .join(person_display_name)
        .join("_pending")
}

/// Parse `mr-{iid}-round-{round}.md` → `(mr_iid, review_round)`.
fn parse_pending_snippet_filename(filename: &str) -> Option<(i64, i64)> {
    let stem = filename.strip_suffix(".md")?;
    let rest = stem.strip_prefix("mr-")?;
    let (iid_str, round_part) = rest.split_once("-round-")?;
    let mr_iid: i64 = iid_str.parse().ok()?;
    let review_round: i64 = round_part.parse().ok()?;
    if mr_iid <= 0 || review_round <= 0 {
        return None;
    }
    Some((mr_iid, review_round))
}

fn observation_status_rank(status: &str) -> u8 {
    match status {
        "published" => 0,
        "draft" => 1,
        "ignored" => 2,
        _ => 3,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pending_snippet_filename_accepts_valid() {
        assert_eq!(
            parse_pending_snippet_filename("mr-4-round-1.md"),
            Some((4, 1))
        );
        assert_eq!(
            parse_pending_snippet_filename("mr-42-round-3.md"),
            Some((42, 3))
        );
    }

    #[test]
    fn parse_pending_snippet_filename_rejects_invalid() {
        assert_eq!(parse_pending_snippet_filename("notes.md"), None);
        assert_eq!(parse_pending_snippet_filename("mr-4.md"), None);
        assert_eq!(parse_pending_snippet_filename("mr-4-round-x.md"), None);
        assert_eq!(parse_pending_snippet_filename("mr-0-round-1.md"), None);
    }

    #[test]
    fn observation_status_rank_orders_published_first() {
        assert!(observation_status_rank("published") < observation_status_rank("draft"));
        assert!(observation_status_rank("draft") < observation_status_rank("ignored"));
        assert!(observation_status_rank("ignored") < observation_status_rank("unknown"));
    }
}
