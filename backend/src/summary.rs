use std::path::{Path, PathBuf};

use serde::Deserialize;
use sqlx::SqlitePool;

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
    pub summary_path: PathBuf,
}

pub fn parse_summary_file(path: &Path) -> Result<ParsedSummary, Error> {
    let content = std::fs::read_to_string(path)?;
    let (yaml, body) = split_frontmatter(&content)?;
    let frontmatter: SummaryFrontmatter =
        serde_yaml::from_str(yaml).map_err(|err| Error::SummaryParse(err.to_string()))?;
    let pending_questions = extract_pending_questions(body);
    Ok(ParsedSummary {
        frontmatter,
        pending_questions,
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
            upsert_summary(pool, project_id, run_id, &parsed).await?;
        }
    }

    Ok(())
}

async fn upsert_summary(
    pool: &SqlitePool,
    project_id: i64,
    run_id: i64,
    parsed: &ParsedSummary,
) -> Result<(), Error> {
    let person_id = upsert_person(pool, &parsed.frontmatter.person).await?;

    let report_date = parsed.frontmatter.date.clone();
    let report_dir = parsed
        .summary_path
        .parent()
        .ok_or_else(|| Error::SummaryParse("summary path has no parent".into()))?;
    let report_md_path = report_dir.join("report.md");
    let summary_md_path = parsed.summary_path.clone();

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
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    let report_id: i64 = sqlx::query_scalar(
        "SELECT id FROM reports WHERE project_id = ? AND person_id = ? AND report_date = ?",
    )
    .bind(project_id)
    .bind(person_id)
    .bind(&report_date)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;

    let raised_date = report_date.get(0..7).unwrap_or(&report_date).to_string();
    for question in &parsed.pending_questions {
        sqlx::query(
            "INSERT INTO pending_items (person_id, project_id, report_id, question, raised_date)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(person_id)
        .bind(project_id)
        .bind(report_id)
        .bind(question)
        .bind(&raised_date)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    }

    Ok(())
}

async fn upsert_person(pool: &SqlitePool, display_name: &str) -> Result<i64, Error> {
    if let Some(person_id) = sqlx::query_scalar("SELECT id FROM people WHERE display_name = ?")
        .bind(display_name)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?
    {
        return Ok(person_id);
    }

    let result = sqlx::query("INSERT INTO people (display_name) VALUES (?)")
        .bind(display_name)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(result.last_insert_rowid())
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

fn extract_pending_questions(body: &str) -> Vec<String> {
    let mut in_section = false;
    let mut questions = Vec::new();

    for line in body.lines() {
        if line.starts_with("## ") {
            in_section = line.trim() == "## 待確認";
            continue;
        }
        if in_section {
            let item = line.trim();
            if let Some(question) = item.strip_prefix("- ") {
                questions.push(question.trim().to_string());
            }
        }
    }

    questions
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
