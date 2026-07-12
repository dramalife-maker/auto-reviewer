use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::process::Command;
use tracing::warn;

use crate::config::{AppConfig, ReviewerAgent};
use crate::executor::execute_agent_turn;
use crate::identity;
use crate::runs;
use crate::worktree::{supply_worktree, WorktreeKind};
use crate::Error;

const TRIAGE_SCRIPT_ENV: &str = "REVIEWER_TRIAGE_SCRIPT";
/// Must stay in sync with `scripts/triage-mrs.py` (`AI_AGENT_MARKER`).
pub const AI_AGENT_MARKER: &str = "By: AI Agent";
pub const SKIP_INBOX_DRAFT: &str = "inbox_draft";
pub const SKIP_INBOX_IGNORED: &str = "inbox_ignored";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EligibleMr {
    pub mr_iid: i64,
    pub mr_title: String,
    pub source_branch: String,
    pub author_identity: String,
    pub review_round: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SkippedMr {
    pub mr_iid: i64,
    pub skip_reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EligibleMrsFile {
    #[serde(default)]
    pub generated_at: Option<String>,
    pub eligible: Vec<EligibleMr>,
    #[serde(default)]
    pub skipped: Vec<SkippedMr>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct InboxSkippedEligible {
    pub mr: EligibleMr,
    pub skip_reason: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MrReviewListItem {
    pub id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub person_id: Option<i64>,
    pub author_name: Option<String>,
    pub mr_iid: i64,
    pub mr_title: Option<String>,
    pub review_round: i64,
    pub status: String,
    pub draft_body: String,
    pub agent_session_id: Option<String>,
    pub reviewer_agent: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DraftFrontmatter {
    pub mr_iid: i64,
    pub mr_title: Option<String>,
    pub review_round: i64,
    pub author_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishResponse {
    pub published_at: String,
    pub published_body: String,
}

/// Published AI reviews for an MR, oldest round first (for follow-up review context).
pub async fn load_prior_published_reviews(
    pool: &SqlitePool,
    project_id: i64,
    mr_iid: i64,
) -> Result<Vec<crate::runs::PriorPublishedReview>, Error> {
    #[derive(Debug, sqlx::FromRow)]
    struct Row {
        review_round: i64,
        published_at: Option<String>,
        published_body: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT review_round, published_at, published_body
        FROM mr_reviews
        WHERE project_id = ?
          AND mr_iid = ?
          AND status = 'published'
          AND published_body IS NOT NULL
          AND trim(published_body) != ''
        ORDER BY review_round ASC
        "#,
    )
    .bind(project_id)
    .bind(mr_iid)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let body = row.published_body?.trim().to_string();
            if body.is_empty() {
                return None;
            }
            Some(crate::runs::PriorPublishedReview {
                review_round: row.review_round,
                published_at: row.published_at,
                body,
            })
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentTurnResponse {
    pub reply: String,
    pub agent_session_id: String,
}

pub fn triage_script_path(config: &AppConfig) -> PathBuf {
    std::env::var(TRIAGE_SCRIPT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| config.app_root().join("scripts").join("triage-mrs.py"))
}

pub async fn run_triage_script(
    config: &AppConfig,
    manifest_path: &Path,
    cwd: &Path,
) -> Result<(), String> {
    let script = triage_script_path(config);
    if !script.is_file() {
        return Err(format!("triage script not found: {}", script.display()));
    }

    let python = which::which("python")
        .or_else(|_| which::which("python3"))
        .map_err(|_| "python not found on PATH".to_string())?;

    let output = Command::new(python)
        .arg(&script)
        .arg("--manifest")
        .arg(manifest_path)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|err| format!("triage script spawn failed: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!("triage script exited with {}", output.status)
        } else {
            stderr.trim().to_string()
        });
    }

    Ok(())
}

pub fn read_eligible_mrs(path: &Path) -> Result<EligibleMrsFile, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("read eligible_mrs.json: {err}"))?;
    serde_json::from_str(&raw).map_err(|err| format!("parse eligible_mrs.json: {err}"))
}

pub async fn load_inbox_blocked_rounds(
    pool: &SqlitePool,
    project_id: i64,
) -> Result<HashMap<(i64, i64), String>, Error> {
    #[derive(Debug, sqlx::FromRow)]
    struct Row {
        mr_iid: i64,
        review_round: i64,
        status: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT mr_iid, review_round, status FROM mr_reviews
         WHERE project_id = ? AND status IN ('draft', 'ignored')",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut blocked = HashMap::new();
    for row in rows {
        let skip_reason = match row.status.as_str() {
            "draft" => SKIP_INBOX_DRAFT.to_string(),
            "ignored" => SKIP_INBOX_IGNORED.to_string(),
            _ => continue,
        };
        blocked.insert((row.mr_iid, row.review_round), skip_reason);
    }
    Ok(blocked)
}

pub fn filter_eligible_by_inbox(
    eligible: &[EligibleMr],
    blocked: &HashMap<(i64, i64), String>,
    force: bool,
) -> (Vec<EligibleMr>, Vec<InboxSkippedEligible>) {
    if force {
        return (eligible.to_vec(), Vec::new());
    }

    let mut to_run = Vec::with_capacity(eligible.len());
    let mut skipped = Vec::new();
    for mr in eligible {
        let key = (mr.mr_iid, mr.review_round);
        if let Some(skip_reason) = blocked.get(&key) {
            skipped.push(InboxSkippedEligible {
                mr: mr.clone(),
                skip_reason: skip_reason.clone(),
            });
        } else {
            to_run.push(mr.clone());
        }
    }
    (to_run, skipped)
}

pub fn append_ai_agent_marker(body: &str) -> String {
    if body_has_ai_agent_footer(body) {
        return body.to_string();
    }
    let trimmed = body.trim_end();
    if trimmed.is_empty() {
        AI_AGENT_MARKER.to_string()
    } else {
        format!("{trimmed}\n\n{AI_AGENT_MARKER}")
    }
}

/// True when the note already ends with the triage dedup footer (not merely mentioned in prose).
pub fn body_has_ai_agent_footer(body: &str) -> bool {
    body.trim_end().ends_with(AI_AGENT_MARKER)
}

pub fn persist_inbox_gate_result(
    path: &Path,
    triage: &EligibleMrsFile,
    to_run: &[EligibleMr],
    inbox_skipped: &[InboxSkippedEligible],
) -> Result<(), String> {
    let mut skipped = triage.skipped.clone();
    for entry in inbox_skipped {
        skipped.push(SkippedMr {
            mr_iid: entry.mr.mr_iid,
            skip_reason: entry.skip_reason.clone(),
        });
    }
    let updated = EligibleMrsFile {
        generated_at: triage.generated_at.clone(),
        eligible: to_run.to_vec(),
        skipped,
    };
    let json = serde_json::to_string_pretty(&updated)
        .map_err(|err| format!("serialize eligible_mrs.json: {err}"))?;
    std::fs::write(path, json).map_err(|err| format!("write eligible_mrs.json: {err}"))
}

pub fn parse_draft_frontmatter(content: &str) -> Option<DraftFrontmatter> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let yaml = rest[..end].trim();
    if yaml.is_empty() {
        return None;
    }

    let mut mr_iid = None;
    let mut mr_title = None;
    let mut review_round = None;
    let mut author_identity = None;

    for line in yaml.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key {
            "mr_iid" => mr_iid = value.parse().ok(),
            "mr_title" => mr_title = Some(value.to_string()),
            "review_round" => review_round = value.parse().ok(),
            "author_identity" => author_identity = Some(value.to_string()),
            _ => {}
        }
    }

    Some(DraftFrontmatter {
        mr_iid: mr_iid?,
        mr_title,
        review_round: review_round?,
        author_identity,
    })
}

/// Markdown body for humans / GitLab notes — YAML frontmatter stripped when present.
pub fn strip_draft_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }
    let rest = &trimmed[3..];
    let Some(end) = rest.find("\n---") else {
        return content;
    };
    let after = &rest[end + 4..];
    after.trim_start_matches(['\r', '\n'])
}

fn yaml_scalar(value: &str) -> String {
    if value.is_empty()
        || value.bytes().any(|b| {
            matches!(
                b,
                b':' | b'#' | b'"' | b'\'' | b'\n' | b'{' | b'}' | b'[' | b']' | b','
            ) || b.is_ascii_whitespace()
        })
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// Rebuild on-disk draft markdown: machine frontmatter + human body.
pub fn compose_draft_markdown(frontmatter: &DraftFrontmatter, body: &str) -> String {
    let mut lines = Vec::with_capacity(8);
    lines.push("---".to_string());
    lines.push(format!("mr_iid: {}", frontmatter.mr_iid));
    if let Some(title) = frontmatter.mr_title.as_deref() {
        lines.push(format!("mr_title: {}", yaml_scalar(title)));
    }
    lines.push(format!("review_round: {}", frontmatter.review_round));
    if let Some(identity) = frontmatter.author_identity.as_deref() {
        lines.push(format!("author_identity: {}", yaml_scalar(identity)));
    }
    lines.push("---".to_string());
    let body = strip_draft_frontmatter(body).trim_start_matches('\n');
    if body.is_empty() {
        lines.join("\n") + "\n"
    } else {
        format!("{}\n\n{}\n", lines.join("\n"), body.trim_end())
    }
}

pub async fn upsert_from_draft_dir(
    pool: &SqlitePool,
    project_id: i64,
    draft_dir: &Path,
    agent_session_id: Option<&str>,
    reviewer_agent: ReviewerAgent,
    revive_inbox: bool,
) -> Result<(), Error> {
    if !draft_dir.is_dir() {
        return Ok(());
    }

    let entries = std::fs::read_dir(draft_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        ingest_mr_draft(
            pool,
            project_id,
            &path,
            agent_session_id,
            reviewer_agent,
            revive_inbox,
        )
        .await?;
    }
    Ok(())
}

pub async fn ingest_mr_draft(
    pool: &SqlitePool,
    project_id: i64,
    draft_path: &Path,
    agent_session_id: Option<&str>,
    reviewer_agent: ReviewerAgent,
    revive_inbox: bool,
) -> Result<(), Error> {
    let content = std::fs::read_to_string(draft_path)?;
    let Some(frontmatter) = parse_draft_frontmatter(&content) else {
        warn!(
            draft = %draft_path.display(),
            "skipping draft with missing or invalid frontmatter"
        );
        return Ok(());
    };

    if frontmatter.mr_iid <= 0 || frontmatter.review_round <= 0 {
        warn!(
            draft = %draft_path.display(),
            "skipping draft missing mr_iid or review_round"
        );
        return Ok(());
    }

    let person_id = if let Some(identity) = frontmatter.author_identity.as_deref() {
        resolve_person_id(pool, identity).await?
    } else {
        None
    };

    let draft_md_path = draft_path.display().to_string();
    let reviewer_agent_str = reviewer_agent.as_str();
    let revive = i64::from(revive_inbox);

    sqlx::query(
        r#"
        INSERT INTO mr_reviews (
            project_id, person_id, mr_iid, mr_title, review_round,
            draft_md_path, status, agent_session_id, reviewer_agent,
            created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?, datetime('now'), datetime('now'))
        ON CONFLICT(project_id, mr_iid, review_round) DO UPDATE SET
            person_id = excluded.person_id,
            mr_title = excluded.mr_title,
            draft_md_path = excluded.draft_md_path,
            agent_session_id = excluded.agent_session_id,
            reviewer_agent = excluded.reviewer_agent,
            status = CASE WHEN ? = 1 THEN 'draft' ELSE mr_reviews.status END,
            published_at = CASE WHEN ? = 1 THEN NULL ELSE mr_reviews.published_at END,
            published_body = CASE WHEN ? = 1 THEN NULL ELSE mr_reviews.published_body END,
            updated_at = datetime('now')
        "#,
    )
    .bind(project_id)
    .bind(person_id)
    .bind(frontmatter.mr_iid)
    .bind(frontmatter.mr_title)
    .bind(frontmatter.review_round)
    .bind(&draft_md_path)
    .bind(agent_session_id)
    .bind(reviewer_agent_str)
    .bind(revive)
    .bind(revive)
    .bind(revive)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    Ok(())
}

async fn resolve_person_id(pool: &SqlitePool, author_identity: &str) -> Result<Option<i64>, Error> {
    let trimmed = author_identity.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if trimmed.contains('@') {
        if let Some(person) = identity::resolve_person_by_email(pool, trimmed).await? {
            return Ok(Some(person.person_id));
        }
    }

    for kind in ["gitlab_user", "glab_user", identity::KIND_GIT_EMAIL] {
        let value = if kind == identity::KIND_GIT_EMAIL {
            identity::normalize_git_email(trimmed)
        } else {
            trimmed.to_string()
        };
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT person_id FROM person_identities WHERE kind = ? AND value = ?",
        )
        .bind(kind)
        .bind(&value)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
        if let Some(person_id) = row {
            return Ok(Some(person_id));
        }
    }

    Ok(None)
}

pub async fn list_mr_reviews(
    pool: &SqlitePool,
    status: Option<&str>,
) -> Result<Vec<MrReviewListItem>, Error> {
    let status = status.unwrap_or("draft");
    let rows = sqlx::query_as::<_, MrReviewRow>(
        r#"
        SELECT mr.id, mr.project_id, p.name AS project_name, mr.person_id,
               pe.display_name AS author_name, mr.mr_iid, mr.mr_title,
               mr.review_round, mr.status, mr.draft_md_path,
               mr.agent_session_id, mr.reviewer_agent, mr.created_at
        FROM mr_reviews mr
        INNER JOIN projects p ON p.id = mr.project_id
        LEFT JOIN people pe ON pe.id = mr.person_id
        WHERE mr.status = ?
        ORDER BY mr.created_at DESC
        "#,
    )
    .bind(status)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let raw = std::fs::read_to_string(&row.draft_md_path).unwrap_or_default();
        items.push(MrReviewListItem {
            id: row.id,
            project_id: row.project_id,
            project_name: row.project_name,
            person_id: row.person_id,
            author_name: row.author_name,
            mr_iid: row.mr_iid,
            mr_title: row.mr_title,
            review_round: row.review_round,
            status: row.status,
            draft_body: strip_draft_frontmatter(&raw).to_string(),
            agent_session_id: row.agent_session_id,
            reviewer_agent: row.reviewer_agent,
            created_at: row.created_at,
        });
    }
    Ok(items)
}

#[derive(Debug, sqlx::FromRow)]
struct MrReviewRow {
    id: i64,
    project_id: i64,
    project_name: String,
    person_id: Option<i64>,
    author_name: Option<String>,
    mr_iid: i64,
    mr_title: Option<String>,
    review_round: i64,
    status: String,
    draft_md_path: String,
    agent_session_id: Option<String>,
    reviewer_agent: String,
    created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MrReviewDetailRow {
    project_id: i64,
    project_name: String,
    mr_iid: i64,
    mr_title: Option<String>,
    review_round: i64,
    status: String,
    draft_md_path: String,
    agent_session_id: Option<String>,
    reviewer_agent: String,
}

async fn load_mr_review(pool: &SqlitePool, id: i64) -> Result<MrReviewDetailRow, Error> {
    sqlx::query_as::<_, MrReviewDetailRow>(
        r#"
        SELECT mr.project_id, p.name AS project_name, mr.mr_iid, mr.mr_title, mr.review_round,
               mr.status, mr.draft_md_path, mr.agent_session_id, mr.reviewer_agent
        FROM mr_reviews mr
        INNER JOIN projects p ON p.id = mr.project_id
        WHERE mr.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?
    .ok_or(Error::NotFound)
}

pub async fn update_draft(pool: &SqlitePool, id: i64, body: &str) -> Result<(), Error> {
    let row = load_mr_review(pool, id).await?;
    if row.status != "draft" {
        return Err(Error::MrReviewConflict);
    }

    let existing = std::fs::read_to_string(&row.draft_md_path).unwrap_or_default();
    let frontmatter = parse_draft_frontmatter(&existing).unwrap_or(DraftFrontmatter {
        mr_iid: row.mr_iid,
        mr_title: row.mr_title.clone(),
        review_round: row.review_round,
        author_identity: None,
    });
    let file_body = compose_draft_markdown(&frontmatter, body);
    std::fs::write(&row.draft_md_path, file_body)?;

    sqlx::query("UPDATE mr_reviews SET updated_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(())
}

pub async fn publish(
    pool: &SqlitePool,
    _config: &AppConfig,
    id: i64,
) -> Result<PublishResponse, Error> {
    let row = load_mr_review(pool, id).await?;
    if row.status != "draft" {
        return Err(Error::MrReviewConflict);
    }

    let draft_raw = std::fs::read_to_string(&row.draft_md_path)?;
    let draft_body = strip_draft_frontmatter(&draft_raw);
    let posted_body = append_ai_agent_marker(draft_body);
    let working_dir = resolve_project_resident_worktree(pool, &row.project_name, &row.project_id)
        .await?;

    glab_mr_note(&working_dir, row.mr_iid, &posted_body).await?;

    let published_at: String = sqlx::query_scalar("SELECT datetime('now')")
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

    sqlx::query(
        r#"
        UPDATE mr_reviews
        SET status = 'published', published_at = ?, published_body = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(&published_at)
    .bind(&posted_body)
    .bind(id)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    Ok(PublishResponse {
        published_at,
        published_body: posted_body,
    })
}

pub async fn ignore(pool: &SqlitePool, id: i64) -> Result<(), Error> {
    let row = load_mr_review(pool, id).await?;
    if row.status != "draft" {
        return Err(Error::MrReviewConflict);
    }
    sqlx::query(
        "UPDATE mr_reviews SET status = 'ignored', updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

pub async fn agent_turn(
    pool: &SqlitePool,
    config: &AppConfig,
    id: i64,
    message: &str,
) -> Result<AgentTurnResponse, Error> {
    let row = load_mr_review(pool, id).await?;
    if row.status != "draft" {
        return Err(Error::MrReviewConflict);
    }
    let session_id = row
        .agent_session_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or(Error::MrReviewConflict)?;

    let working_dir = resolve_project_resident_worktree(pool, &row.project_name, &row.project_id)
        .await?;
    let agent = ReviewerAgent::parse_db_value(&row.reviewer_agent);
    let (reply, new_session_id) =
        execute_agent_turn(config, &working_dir, session_id, message, agent).await?;

    let session_id = new_session_id.unwrap_or_else(|| session_id.to_string());
    sqlx::query("UPDATE mr_reviews SET agent_session_id = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&session_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

    Ok(AgentTurnResponse {
        reply,
        agent_session_id: session_id,
    })
}

async fn resolve_project_resident_worktree(
    pool: &SqlitePool,
    project_name: &str,
    project_id: &i64,
) -> Result<PathBuf, Error> {
    let (is_git_repo, default_branch) = crate::projects::get_project(pool, project_name).await?;
    if is_git_repo == 0 {
        return Err(Error::InvalidProjectConfig(
            "project is not provisioned".into(),
        ));
    }
    let branch = default_branch.ok_or_else(|| {
        Error::InvalidProjectConfig("no default branch for project".into())
    })?;
    let repo_path = runs::load_mr_poll_project(pool, *project_id)
        .await?
        .map(|row| row.repo_path)
        .ok_or(Error::NotFound)?;
    supply_worktree(Path::new(&repo_path), &branch, WorktreeKind::Resident)
        .await
        .map_err(|err| Error::InvalidProjectConfig(err.to_string()))
}

/// Relative path under `report_root` for an MR observation snippet file.
pub fn pending_snippet_relative_path(
    person_folder: &str,
    mr_iid: i64,
    review_round: i64,
) -> String {
    format!("{person_folder}/_pending/mr-{mr_iid}-round-{review_round}.md")
}

/// Paths (relative to `report_root`) for published MR observation snippets the weekly
/// batch may fold into `summary.md`. When multiple rounds are published for one MR,
/// only the latest round is included.
pub async fn load_published_pending_snippets(
    pool: &SqlitePool,
    project_id: i64,
) -> Result<Vec<String>, Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        mr_iid: i64,
        review_round: i64,
        person_folder: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT mr.mr_iid, mr.review_round, p.display_name AS person_folder
         FROM mr_reviews mr
         LEFT JOIN people p ON p.id = mr.person_id
         WHERE mr.project_id = ? AND mr.status = 'published'
         ORDER BY mr.mr_iid ASC, mr.review_round DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let mut snippets = Vec::new();
    let mut seen_mr = std::collections::HashSet::new();
    for row in rows {
        if !seen_mr.insert(row.mr_iid) {
            continue;
        }
        let Some(person_folder) = row.person_folder else {
            warn!(
                mr_iid = row.mr_iid,
                "skipping published snippet without resolved person folder"
            );
            continue;
        };
        snippets.push(pending_snippet_relative_path(
            &person_folder,
            row.mr_iid,
            row.review_round,
        ));
    }
    Ok(snippets)
}

async fn glab_mr_note(cwd: &Path, mr_iid: i64, message: &str) -> Result<(), Error> {
    let output = Command::new("glab")
        .args([
            "mr",
            "note",
            &mr_iid.to_string(),
            "--message",
            message,
        ])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|err| Error::AgentFailed(format!("glab spawn failed: {err}")))?;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::AgentFailed(if stderr.trim().is_empty() {
        format!("glab mr note failed with {}", output.status)
    } else {
        stderr.trim().to_string()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_draft_frontmatter_reads_required_fields() {
        let content = r#"---
mr_iid: 42
mr_title: feat cache
review_round: 1
author_identity: alice@co.com
---
Body here
"#;
        let parsed = parse_draft_frontmatter(content).expect("frontmatter");
        assert_eq!(parsed.mr_iid, 42);
        assert_eq!(parsed.mr_title.as_deref(), Some("feat cache"));
        assert_eq!(parsed.review_round, 1);
        assert_eq!(parsed.author_identity.as_deref(), Some("alice@co.com"));
    }

    #[test]
    fn parse_draft_frontmatter_returns_none_without_mr_iid() {
        let content = "---\nreview_round: 1\n---\n";
        assert!(parse_draft_frontmatter(content).is_none());
    }

    #[test]
    fn strip_draft_frontmatter_returns_human_body() {
        let content = r#"---
mr_iid: 68
mr_title: "feat wallet"
review_round: 1
author_identity: garytsai
---

# MR !68

Body text
"#;
        assert_eq!(
            strip_draft_frontmatter(content),
            "# MR !68\n\nBody text\n"
        );
        assert_eq!(strip_draft_frontmatter("no yaml here"), "no yaml here");
    }

    #[test]
    fn compose_draft_markdown_round_trips_body_without_yaml_in_strip() {
        let fm = DraftFrontmatter {
            mr_iid: 68,
            mr_title: Some(":sparkles: feat".into()),
            review_round: 1,
            author_identity: Some("garytsai".into()),
        };
        let file = compose_draft_markdown(&fm, "# Hello\n\nWorld");
        assert!(file.starts_with("---\n"));
        assert!(file.contains("mr_iid: 68\n"));
        assert_eq!(strip_draft_frontmatter(&file).trim(), "# Hello\n\nWorld");
        // Saving a body that already includes frontmatter must not nest it.
        let again = compose_draft_markdown(&fm, &file);
        assert_eq!(strip_draft_frontmatter(&again).trim(), "# Hello\n\nWorld");
        assert_eq!(again.matches("mr_iid:").count(), 1);
    }

    #[test]
    fn append_ai_agent_marker_on_stripped_body_omits_yaml() {
        let content = "---\nmr_iid: 1\nreview_round: 1\n---\n\nReview body\n";
        let posted = append_ai_agent_marker(strip_draft_frontmatter(content));
        assert!(!posted.contains("mr_iid:"));
        assert!(posted.starts_with("Review body"));
        assert!(posted.trim_end().ends_with(AI_AGENT_MARKER));
    }

    #[test]
    fn pending_snippet_relative_path_uses_person_folder() {
        assert_eq!(
            pending_snippet_relative_path("Alice Chen", 42, 1),
            "Alice Chen/_pending/mr-42-round-1.md"
        );
    }

    #[test]
    fn filter_eligible_by_inbox_skips_draft_and_ignored() {
        let eligible = vec![
            EligibleMr {
                mr_iid: 10,
                mr_title: "a".into(),
                source_branch: "feat/a".into(),
                author_identity: "alice".into(),
                review_round: 1,
            },
            EligibleMr {
                mr_iid: 11,
                mr_title: "b".into(),
                source_branch: "feat/b".into(),
                author_identity: "bob".into(),
                review_round: 2,
            },
            EligibleMr {
                mr_iid: 12,
                mr_title: "c".into(),
                source_branch: "feat/c".into(),
                author_identity: "carol".into(),
                review_round: 1,
            },
        ];
        let mut blocked = HashMap::new();
        blocked.insert((10, 1), SKIP_INBOX_DRAFT.to_string());
        blocked.insert((11, 2), SKIP_INBOX_IGNORED.to_string());

        let (to_run, skipped) = filter_eligible_by_inbox(&eligible, &blocked, false);
        assert_eq!(to_run.len(), 1);
        assert_eq!(to_run[0].mr_iid, 12);
        assert_eq!(skipped.len(), 2);
        assert_eq!(skipped[0].skip_reason, SKIP_INBOX_DRAFT);
        assert_eq!(skipped[1].skip_reason, SKIP_INBOX_IGNORED);
    }

    #[test]
    fn filter_eligible_by_inbox_force_bypasses_blocked() {
        let eligible = vec![EligibleMr {
            mr_iid: 10,
            mr_title: "a".into(),
            source_branch: "feat/a".into(),
            author_identity: "alice".into(),
            review_round: 1,
        }];
        let mut blocked = HashMap::new();
        blocked.insert((10, 1), SKIP_INBOX_DRAFT.to_string());

        let (to_run, skipped) = filter_eligible_by_inbox(&eligible, &blocked, true);
        assert_eq!(to_run.len(), 1);
        assert!(skipped.is_empty());
    }

    #[test]
    fn append_ai_agent_marker_adds_footer_when_missing() {
        assert_eq!(
            append_ai_agent_marker("## Review\n\nLooks good."),
            "## Review\n\nLooks good.\n\nBy: AI Agent"
        );
    }

    #[test]
    fn append_ai_agent_marker_does_not_duplicate() {
        let body = "Summary\n\nBy: AI Agent";
        assert_eq!(append_ai_agent_marker(body), body);
    }

    #[test]
    fn append_ai_agent_marker_adds_footer_when_marker_only_in_prose() {
        let body = "Please note: By: AI Agent is our footer format.\n\nLooks good.";
        assert_eq!(
            append_ai_agent_marker(body),
            "Please note: By: AI Agent is our footer format.\n\nLooks good.\n\nBy: AI Agent"
        );
    }

    #[test]
    fn persist_inbox_gate_result_writes_skipped_and_eligible() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("eligible_mrs.json");
        let triage = EligibleMrsFile {
            generated_at: Some("2026-07-09T00:00:00Z".into()),
            eligible: vec![EligibleMr {
                mr_iid: 10,
                mr_title: "a".into(),
                source_branch: "feat/a".into(),
                author_identity: "alice".into(),
                review_round: 1,
            }],
            skipped: vec![SkippedMr {
                mr_iid: 9,
                skip_reason: "gitlab_draft".into(),
            }],
        };
        std::fs::write(&path, serde_json::to_string_pretty(&triage).unwrap()).expect("seed");

        let inbox_skipped = vec![InboxSkippedEligible {
            mr: triage.eligible[0].clone(),
            skip_reason: SKIP_INBOX_DRAFT.to_string(),
        }];
        persist_inbox_gate_result(&path, &triage, &[], &inbox_skipped).expect("persist");

        let updated = read_eligible_mrs(&path).expect("read");
        assert!(updated.eligible.is_empty());
        assert_eq!(updated.skipped.len(), 2);
        assert!(updated
            .skipped
            .iter()
            .any(|row| row.skip_reason == SKIP_INBOX_DRAFT));
    }

    #[tokio::test]
    async fn load_inbox_blocked_rounds_returns_draft_and_ignored() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pool = crate::db::init_pool(temp.path()).await.expect("init pool");

        sqlx::query(
            "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
        )
        .bind(temp.path().join("repos/alpha").display().to_string())
        .execute(&pool)
        .await
        .expect("insert project");

        let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
            .fetch_one(&pool)
            .await
            .expect("project id");

        let draft_dir = temp.path().join("drafts");
        std::fs::create_dir_all(&draft_dir).expect("draft dir");
        for (mr_iid, status) in [(10, "draft"), (11, "ignored")] {
            let draft_path = draft_dir.join(format!("mr-{mr_iid}.md"));
            std::fs::write(
                &draft_path,
                format!(
                    "---\nmr_iid: {mr_iid}\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody\n"
                ),
            )
            .expect("write draft");
            ingest_mr_draft(
                &pool,
                project_id,
                &draft_path,
                None,
                ReviewerAgent::Cursor,
                false,
            )
            .await
            .expect("ingest");
            if status == "ignored" {
                sqlx::query("UPDATE mr_reviews SET status = 'ignored' WHERE mr_iid = ?")
                    .bind(mr_iid)
                    .execute(&pool)
                    .await
                    .expect("ignore");
            }
        }

        let blocked = load_inbox_blocked_rounds(&pool, project_id)
            .await
            .expect("blocked");
        assert_eq!(blocked.get(&(10, 1)), Some(&SKIP_INBOX_DRAFT.to_string()));
        assert_eq!(blocked.get(&(11, 1)), Some(&SKIP_INBOX_IGNORED.to_string()));
    }

    #[tokio::test]
    async fn inbox_gate_blocks_all_eligible_when_draft_exists() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pool = crate::db::init_pool(temp.path()).await.expect("init pool");

        sqlx::query(
            "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
        )
        .bind(temp.path().join("repos/alpha").display().to_string())
        .execute(&pool)
        .await
        .expect("insert project");

        let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
            .fetch_one(&pool)
            .await
            .expect("project id");

        let draft_dir = temp.path().join("drafts");
        std::fs::create_dir_all(&draft_dir).expect("draft dir");
        let draft_path = draft_dir.join("mr-42.md");
        std::fs::write(
            &draft_path,
            "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody\n",
        )
        .expect("write draft");
        ingest_mr_draft(
            &pool,
            project_id,
            &draft_path,
            None,
            ReviewerAgent::Cursor,
            false,
        )
        .await
        .expect("ingest");

        let eligible = vec![EligibleMr {
            mr_iid: 42,
            mr_title: "t".into(),
            source_branch: "feat/x".into(),
            author_identity: "a@b.com".into(),
            review_round: 1,
        }];
        let blocked = load_inbox_blocked_rounds(&pool, project_id)
            .await
            .expect("blocked");
        let (to_run, skipped) = filter_eligible_by_inbox(&eligible, &blocked, false);
        assert!(to_run.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].skip_reason, SKIP_INBOX_DRAFT);
    }

    #[tokio::test]
    async fn force_reingest_revives_ignored_row_to_draft() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pool = crate::db::init_pool(temp.path()).await.expect("init pool");

        sqlx::query(
            "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
        )
        .bind(temp.path().join("repos/alpha").display().to_string())
        .execute(&pool)
        .await
        .expect("insert project");

        let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
            .fetch_one(&pool)
            .await
            .expect("project id");

        let draft_dir = temp.path().join("drafts");
        std::fs::create_dir_all(&draft_dir).expect("draft dir");
        let draft_path = draft_dir.join("mr-42.md");
        std::fs::write(
            &draft_path,
            "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nignored body\n",
        )
        .expect("write draft");

        ingest_mr_draft(
            &pool,
            project_id,
            &draft_path,
            Some("sess-1"),
            ReviewerAgent::Cursor,
            false,
        )
        .await
        .expect("ingest");
        sqlx::query("UPDATE mr_reviews SET status = 'ignored' WHERE mr_iid = 42")
            .execute(&pool)
            .await
            .expect("ignore");

        std::fs::write(
            &draft_path,
            "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nforced body\n",
        )
        .expect("rewrite draft");

        ingest_mr_draft(
            &pool,
            project_id,
            &draft_path,
            Some("sess-2"),
            ReviewerAgent::Cursor,
            true,
        )
        .await
        .expect("re-ingest");

        let status: String =
            sqlx::query_scalar("SELECT status FROM mr_reviews WHERE mr_iid = 42")
                .fetch_one(&pool)
                .await
                .expect("status");
        assert_eq!(status, "draft");

        let items = list_mr_reviews(&pool, None).await.expect("list");
        assert_eq!(items.len(), 1);
        assert!(items[0].draft_body.contains("forced body"));
    }
}
