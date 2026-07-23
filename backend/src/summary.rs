use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use tracing::{info, warn};

use crate::identity;
use crate::Error;

#[derive(Debug, Deserialize)]
pub struct SummaryFrontmatter {
    pub person: String,
    pub project: String,
    date: String,
    one_line: Option<String>,
    mr_count: Option<i64>,
    commit_count: Option<i64>,
}

#[derive(Debug)]
pub struct ParsedSummary {
    pub frontmatter: SummaryFrontmatter,
    pub pending_questions: Vec<String>,
    pub resolved_questions: Vec<String>,
    pub highlights: Vec<String>,
    pub growth: Vec<String>,
    pub summary_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct SummarySections {
    pub highlights: Vec<String>,
    pub growth: Vec<String>,
}

impl SummarySections {
    pub fn from_summary_file(path: &Path) -> Result<Self, Error> {
        let parsed = parse_summary_file(path)?;
        Ok(Self {
            highlights: parsed.highlights,
            growth: parsed.growth,
        })
    }
}

pub fn parse_summary_file(path: &Path) -> Result<ParsedSummary, Error> {
    let content = std::fs::read_to_string(path)?;
    let (yaml, body) = split_frontmatter(&content)?;
    let frontmatter: SummaryFrontmatter =
        serde_yaml::from_str(yaml).map_err(|err| Error::SummaryParse(err.to_string()))?;
    Ok(ParsedSummary {
        pending_questions: extract_bullet_section(body, "## 待確認"),
        resolved_questions: extract_bullet_section(body, "## 已釐清"),
        highlights: extract_bullet_section(body, "## 本週重點"),
        growth: extract_bullet_section(body, "## 成長面向"),
        frontmatter,
        summary_path: path.to_path_buf(),
    })
}

pub async fn ingest_project_summaries(
    pool: &SqlitePool,
    data_root: &Path,
    project_name: &str,
    project_id: i64,
    run_id: i64,
) -> Result<(), Error> {
    let report_root = data_root.join("reports").join(project_name);
    if !report_root.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(&report_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let person_dir = entry.path();
        for summary in find_summary_files(&person_dir)? {
            let parsed = parse_summary_file(&summary)?;
            upsert_summary(pool, data_root, project_id, run_id, &parsed).await?;
        }
    }

    Ok(())
}

/// Re-scan `summary.md` files under each `reports/<project>/<folder_name>/` for one person
/// and upsert into DB. Preserves existing `run_id` when a report row already exists; for a new
/// row uses the person's latest `run_id` on that project. Failures are returned as warnings.
pub async fn reingest_person_summaries(
    pool: &SqlitePool,
    data_root: &Path,
    person_id: i64,
    folder_name: &str,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let reports_root = data_root.join("reports");
    if !reports_root.is_dir() {
        return warnings;
    }

    let entries = match std::fs::read_dir(&reports_root) {
        Ok(entries) => entries,
        Err(err) => {
            warnings.push(format!("failed to read reports root: {err}"));
            return warnings;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warnings.push(format!("failed to read reports entry: {err}"));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                warnings.push(format!("failed to stat {}: {err}", entry.path().display()));
                continue;
            }
        };
        if !file_type.is_dir() {
            continue;
        }
        let project_name = entry.file_name();
        let project_name = project_name.to_string_lossy();
        if project_name.starts_with('_') {
            continue;
        }

        let person_dir = entry.path().join(folder_name);
        if !person_dir.is_dir() {
            continue;
        }

        let project_id: Option<i64> =
            match sqlx::query_scalar("SELECT id FROM projects WHERE name = ?")
                .bind(project_name.as_ref())
                .fetch_optional(pool)
                .await
            {
                Ok(id) => id,
                Err(err) => {
                    warnings.push(format!(
                        "failed to look up project {project_name}: {err}"
                    ));
                    continue;
                }
            };
        let Some(project_id) = project_id else {
            warnings.push(format!(
                "skipping summaries under unknown project {project_name}"
            ));
            continue;
        };

        let summaries = match find_summary_files(&person_dir) {
            Ok(files) => files,
            Err(err) => {
                warnings.push(format!(
                    "failed to list summaries for {project_name}/{folder_name}: {err}"
                ));
                continue;
            }
        };

        for summary_path in summaries {
            let parsed = match parse_summary_file(&summary_path) {
                Ok(parsed) => parsed,
                Err(err) => {
                    warnings.push(format!(
                        "failed to parse {}: {err}",
                        summary_path.display()
                    ));
                    continue;
                }
            };

            let run_id = match resolve_reingest_run_id(
                pool,
                project_id,
                person_id,
                &parsed.frontmatter.date,
            )
            .await
            {
                Ok(Some(run_id)) => run_id,
                Ok(None) => {
                    warnings.push(format!(
                        "skipping {}: no existing run_id for person/project",
                        summary_path.display()
                    ));
                    continue;
                }
                Err(err) => {
                    warnings.push(format!(
                        "failed to resolve run_id for {}: {err}",
                        summary_path.display()
                    ));
                    continue;
                }
            };

            if let Err(err) =
                upsert_summary(pool, data_root, project_id, run_id, &parsed).await
            {
                warnings.push(format!(
                    "failed to upsert {}: {err}",
                    summary_path.display()
                ));
            }
        }
    }

    warnings
}

async fn resolve_reingest_run_id(
    pool: &SqlitePool,
    project_id: i64,
    person_id: i64,
    report_date: &str,
) -> Result<Option<i64>, Error> {
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT run_id FROM reports WHERE project_id = ? AND person_id = ? AND report_date = ?",
    )
    .bind(project_id)
    .bind(person_id)
    .bind(report_date)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;
    if existing.is_some() {
        return Ok(existing);
    }

    let latest: Option<i64> = sqlx::query_scalar(
        "SELECT run_id FROM reports
         WHERE project_id = ? AND person_id = ?
         ORDER BY report_date DESC, id DESC
         LIMIT 1",
    )
    .bind(project_id)
    .bind(person_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;
    Ok(latest)
}

async fn upsert_summary(
    pool: &SqlitePool,
    data_root: &Path,
    project_id: i64,
    run_id: i64,
    parsed: &ParsedSummary,
) -> Result<(), Error> {
    // frontmatter `person` carries the immutable folder_name, so ingest resolves
    // by folder_name and keeps working after a display_name rename.
    let Some(person_id) =
        identity::resolve_person_id_by_folder_name(pool, &parsed.frontmatter.person).await?
    else {
        warn!(
            person = %parsed.frontmatter.person,
            summary = %parsed.summary_path.display(),
            "skipping summary: unknown person"
        );
        return Ok(());
    };

    let report_date = parsed.frontmatter.date.clone();
    let report_dir = parsed
        .summary_path
        .parent()
        .ok_or_else(|| Error::SummaryParse("summary path has no parent".into()))?;
    let report_md_path = report_dir.join("report.md");
    let summary_md_path = parsed.summary_path.clone();

    let mut tx = pool.begin().await.map_err(Error::Database)?;

    sqlx::query(
        "INSERT INTO reports (
            project_id, person_id, run_id, report_date, report_md_path, summary_md_path,
            one_line, mr_count, commit_count
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(project_id, person_id, report_date) DO UPDATE SET
            run_id = excluded.run_id,
            report_md_path = excluded.report_md_path,
            summary_md_path = excluded.summary_md_path,
            one_line = excluded.one_line,
            mr_count = excluded.mr_count,
            commit_count = excluded.commit_count",
    )
    .bind(project_id)
    .bind(person_id)
    .bind(run_id)
    .bind(&report_date)
    .bind(report_md_path.display().to_string())
    .bind(summary_md_path.display().to_string())
    .bind(&parsed.frontmatter.one_line)
    .bind(parsed.frontmatter.mr_count)
    .bind(parsed.frontmatter.commit_count)
    .execute(&mut *tx)
    .await
    .map_err(Error::Database)?;

    let report_id: i64 = sqlx::query_scalar(
        "SELECT id FROM reports WHERE project_id = ? AND person_id = ? AND report_date = ?",
    )
    .bind(project_id)
    .bind(person_id)
    .bind(&report_date)
    .fetch_one(&mut *tx)
    .await
    .map_err(Error::Database)?;

    let raised_date = report_date.get(0..7).unwrap_or(&report_date).to_string();
    for question in &parsed.pending_questions {
        // A row whose originating report is gone (`report_id` is NULL, the column
        // is ON DELETE SET NULL) has no date to compare, so the guard below cannot
        // see it and insertion proceeds. Failing open is deliberate: dropping a
        // genuinely new question is worse than an extra row.
        let unanchored: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pending_items
             WHERE person_id = ? AND project_id = ? AND question = ? AND report_id IS NULL",
        )
        .bind(person_id)
        .bind(project_id)
        .bind(question)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;
        if unanchored > 0 {
            warn!(
                person_id,
                project_id,
                question,
                "existing pending item has no originating report; replay guard cannot compare dates"
            );
        }

        // Ingest re-reads every historical summary.md, so an already-processed
        // one reappears on each later run. Skip the insert when this question was
        // already recorded from a report dated at or after the incoming summary —
        // an older mention is history, not news. `>=` rather than `=` because
        // carrying an open question forward rewrites its report_id to the newer
        // report, which would otherwise let the original summary's replay through.
        //
        // `INSERT OR IGNORE` still guards the separate rule that one question may
        // not hold two open rows (idx_pending_open_unique, a partial index over
        // status='open'); that index is what this guard complements, not replaces.
        //
        // report_date is compared lexicographically, matching how it is already
        // ordered elsewhere in this module.
        let insert_result = sqlx::query(
            "INSERT OR IGNORE INTO pending_items (person_id, project_id, report_id, question, raised_date)
             SELECT ?, ?, ?, ?, ?
             WHERE NOT EXISTS (
                 SELECT 1 FROM pending_items existing
                 INNER JOIN reports source ON source.id = existing.report_id
                 WHERE existing.person_id = ?
                   AND existing.project_id = ?
                   AND existing.question = ?
                   AND source.report_date >= ?
             )",
        )
        .bind(person_id)
        .bind(project_id)
        .bind(report_id)
        .bind(question)
        .bind(&raised_date)
        .bind(person_id)
        .bind(project_id)
        .bind(question)
        .bind(&report_date)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        if insert_result.rows_affected() == 0 {
            sqlx::query(
                "UPDATE pending_items
                 SET report_id = ?
                 WHERE person_id = ? AND project_id = ? AND question = ? AND status = 'open'",
            )
            .bind(report_id)
            .bind(person_id)
            .bind(project_id)
            .bind(question)
            .execute(&mut *tx)
            .await
            .map_err(Error::Database)?;
        }
    }

    tx.commit().await.map_err(Error::Database)?;

    for question in &parsed.resolved_questions {
        if parsed.pending_questions.iter().any(|q| q == question) {
            warn!(
                person_id,
                project_id,
                question,
                "question appears in both ## 待確認 and ## 已釐清; resolving open match if any"
            );
        }
        resolve_from_summary_cleared(pool, data_root, person_id, project_id, question).await?;
    }

    Ok(())
}

/// Resolve an open pending item listed under `## 已釐清`. Missing matches are ignored.
/// Notes sync failures are logged and do not abort ingest (DB remains resolved).
async fn resolve_from_summary_cleared(
    pool: &SqlitePool,
    data_root: &Path,
    person_id: i64,
    project_id: i64,
    question: &str,
) -> Result<(), Error> {
    let item_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM pending_items
         WHERE person_id = ? AND project_id = ? AND question = ? AND status = 'open'",
    )
    .bind(person_id)
    .bind(project_id)
    .bind(question)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    let Some(item_id) = item_id else {
        warn!(
            person_id,
            project_id,
            question,
            "skipping 已釐清 bullet with no matching open pending item"
        );
        return Ok(());
    };

    match crate::pending_items::resolve_pending_item(
        pool,
        data_root,
        item_id,
        crate::pending_items::ResolvePendingItemInput {
            status: "resolved".into(),
            resolution_note: None,
        },
    )
    .await
    {
        Ok(_) => Ok(()),
        // pending_items already logged NotesSyncFailed; DB remains resolved — continue ingest.
        Err(Error::NotesSyncFailed(_)) => Ok(()),
        Err(Error::PendingItemAlreadyResolved) | Err(Error::NotFound) => {
            warn!(
                item_id,
                person_id,
                project_id,
                question,
                "已釐清 resolve skipped: item already resolved or missing"
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn find_summary_files(person_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    if !person_dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(person_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let summary = path.join("summary.md");
            if summary.is_file() {
                files.push(summary);
            }
        }
    }
    Ok(files)
}

fn split_frontmatter(content: &str) -> Result<(&str, &str), Error> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(Error::SummaryParse("missing frontmatter".into()));
    }
    let rest = &trimmed[3..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| Error::SummaryParse("unclosed frontmatter".into()))?;
    let yaml = &rest[..end];
    let body = &rest[end + 4..];
    Ok((yaml.trim(), body))
}

fn extract_bullet_section(body: &str, heading: &str) -> Vec<String> {
    let mut in_section = false;
    let mut items = Vec::new();

    for line in body.lines() {
        if line.starts_with("## ") {
            in_section = line.trim() == heading;
            continue;
        }
        if in_section {
            let item = line.trim();
            if let Some(text) = item.strip_prefix("- ") {
                items.push(text.trim().to_string());
            }
        }
    }

    items
}

pub async fn count_reports_for_run(pool: &SqlitePool, run_id: i64) -> Result<i64, Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports WHERE run_id = ?")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(count)
}

pub async fn count_pending_for_person(pool: &SqlitePool, person_name: &str) -> Result<i64, Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items pi
         INNER JOIN people p ON p.id = pi.person_id
         WHERE p.display_name = ? AND pi.status = 'open'",
    )
    .bind(person_name)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;
    Ok(count)
}

#[derive(Debug, sqlx::FromRow)]
struct BackfillReportRow {
    report_id: i64,
    person_id: i64,
    project_id: i64,
    report_date: String,
    summary_md_path: String,
}

/// Seed `pending_items` from existing `summary.md` files for deployments that
/// upgraded before ingest wrote pending rows. Runs at most once per database.
pub async fn backfill_pending_items_if_needed(pool: &SqlitePool) -> Result<(), Error> {
    let already_done: Option<String> =
        sqlx::query_scalar("SELECT value FROM app_meta WHERE key = 'pending_items_backfill_v1'")
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

    if already_done.is_some() {
        return Ok(());
    }

    let inserted = backfill_pending_items(pool).await?;
    sqlx::query("INSERT INTO app_meta (key, value) VALUES ('pending_items_backfill_v1', ?)")
        .bind(inserted.to_string())
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(())
}

/// Seed `pending_items` from existing `summary.md` files.
pub async fn backfill_pending_items(pool: &SqlitePool) -> Result<u64, Error> {
    let rows = sqlx::query_as::<_, BackfillReportRow>(
        "SELECT r.id AS report_id, r.person_id, r.project_id, r.report_date, r.summary_md_path
         FROM reports r
         ORDER BY r.report_date, r.id",
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut inserted = 0u64;
    for row in rows {
        let summary_path = Path::new(&row.summary_md_path);
        if !summary_path.is_file() {
            continue;
        }
        let parsed = match parse_summary_file(summary_path) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    summary = %summary_path.display(),
                    error = %err,
                    "skipping pending backfill for unreadable summary"
                );
                continue;
            }
        };

        let raised_date = row.report_date.get(0..7).unwrap_or(&row.report_date).to_string();
        for question in &parsed.pending_questions {
            let insert_result = sqlx::query(
                "INSERT OR IGNORE INTO pending_items (person_id, project_id, report_id, question, raised_date)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(row.person_id)
            .bind(row.project_id)
            .bind(row.report_id)
            .bind(question)
            .bind(&raised_date)
            .execute(pool)
            .await
            .map_err(Error::Database)?;

            if insert_result.rows_affected() > 0 {
                inserted += 1;
            }
        }
    }

    if inserted > 0 {
        info!(inserted, "backfilled pending_items from summary files");
    }

    Ok(inserted)
}
