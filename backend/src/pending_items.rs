use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::schedule::load_schedule_config;
use crate::Error;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PendingItem {
    pub id: i64,
    pub person_id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub report_id: Option<i64>,
    pub question: String,
    pub status: String,
    pub raised_date: String,
    pub resolved_date: Option<String>,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResolvePendingItemInput {
    pub status: String,
    pub resolution_note: Option<String>,
}

/// List pending items for a person, optionally filtered by status.
/// `status` accepts `open` (default), `resolved`, or `all`.
pub async fn list_pending_items_for_person(
    pool: &SqlitePool,
    person_id: i64,
    status: Option<&str>,
) -> Result<Vec<PendingItem>, Error> {
    let person_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
    if person_exists.is_none() {
        return Err(Error::NotFound);
    }

    let status_filter = status.unwrap_or("open");
    if !matches!(status_filter, "open" | "resolved" | "all") {
        return Err(Error::InvalidPendingItemListStatus);
    }

    let base_query = "SELECT pi.id, pi.person_id, pi.project_id, pr.name AS project_name,
                pi.report_id, pi.question, pi.status, pi.raised_date,
                pi.resolved_date, pi.resolution_note
         FROM pending_items pi
         INNER JOIN projects pr ON pr.id = pi.project_id
         WHERE pi.person_id = ?";

    let rows = match status_filter {
        "all" => {
            sqlx::query_as::<_, PendingItem>(&format!(
                "{base_query} ORDER BY pi.raised_date DESC, pi.id DESC"
            ))
            .bind(person_id)
            .fetch_all(pool)
            .await
        }
        other => {
            sqlx::query_as::<_, PendingItem>(&format!(
                "{base_query} AND pi.status = ? ORDER BY pi.raised_date DESC, pi.id DESC"
            ))
            .bind(person_id)
            .bind(other)
            .fetch_all(pool)
            .await
        }
    };

    rows.map_err(Error::Database)
}

/// Resolve an open pending item. Only `open -> resolved` is supported.
///
/// On success, the person-level `_notes.md` file is synced (DB-first: the
/// database update is committed before the file sync is attempted). If the
/// file sync fails, the database row remains `resolved` and `NotesSyncFailed`
/// is returned so the caller can surface HTTP 502.
pub async fn resolve_pending_item(
    pool: &SqlitePool,
    data_root: &Path,
    item_id: i64,
    input: ResolvePendingItemInput,
) -> Result<PendingItem, Error> {
    if input.status != "resolved" {
        return Err(Error::InvalidPendingItemStatus);
    }

    let resolved_date = current_month_for_schedule(pool).await?;
    let resolution_note = input
        .resolution_note
        .as_deref()
        .map(str::trim)
        .filter(|note| !note.is_empty());

    info!(item_id, "resolving pending item");

    let update_result = sqlx::query(
        "UPDATE pending_items
         SET status = 'resolved', resolved_date = ?, resolution_note = ?
         WHERE id = ? AND status = 'open'",
    )
    .bind(&resolved_date)
    .bind(resolution_note)
    .bind(item_id)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    if update_result.rows_affected() == 0 {
        let current_status: Option<String> =
            sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
                .bind(item_id)
                .fetch_optional(pool)
                .await
                .map_err(Error::Database)?;

        return match current_status {
            None => Err(Error::NotFound),
            Some(_) => Err(Error::PendingItemAlreadyResolved),
        };
    }

    let item = fetch_pending_item(pool, item_id)
        .await?
        .ok_or(Error::NotFound)?;

    let display_name: String = sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
        .bind(item.person_id)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

    let notes_path = crate::person_trends::person_trends_dir(data_root, &display_name)
        .join("_notes.md");
    if let Err(err) = sync_notes_file(&notes_path, &item) {
        warn!(
            item_id,
            person_id = item.person_id,
            error = %err,
            "notes sync failed after pending item resolve"
        );
        return Err(Error::NotesSyncFailed(err.to_string()));
    }

    info!(
        item_id,
        person_id = item.person_id,
        project_id = item.project_id,
        "pending item resolved"
    );

    Ok(item)
}

pub async fn fetch_pending_item(
    pool: &SqlitePool,
    item_id: i64,
) -> Result<Option<PendingItem>, Error> {
    sqlx::query_as::<_, PendingItem>(
        "SELECT pi.id, pi.person_id, pi.project_id, pr.name AS project_name,
                pi.report_id, pi.question, pi.status, pi.raised_date,
                pi.resolved_date, pi.resolution_note
         FROM pending_items pi
         INNER JOIN projects pr ON pr.id = pi.project_id
         WHERE pi.id = ?",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)
}

/// Current calendar month `YYYY-MM` computed using the schedule timezone offset.
async fn current_month_for_schedule(pool: &SqlitePool) -> Result<String, Error> {
    let config = load_schedule_config(pool).await?;
    let offset_secs = (config.tz_offset_min as i32) * 60;
    let tz = chrono::FixedOffset::east_opt(offset_secs).ok_or_else(|| {
        Error::SummaryParse(format!("invalid tz_offset_min: {}", config.tz_offset_min))
    })?;
    let now = Utc::now().with_timezone(&tz);
    Ok(now.format("%Y-%m").to_string())
}

/// Build the resolved B1 line for a pending item: `- [raised→resolved] ✓ question`,
/// with an optional trailing ` — note` when `resolution_note` is present.
fn build_resolved_line(item: &PendingItem) -> String {
    let resolved_month = item.resolved_date.as_deref().unwrap_or_default();
    let mut line = format!(
        "- [{}→{}] ✓ {}",
        item.raised_date, resolved_month, item.question
    );
    if let Some(note) = item.resolution_note.as_deref().filter(|n| !n.is_empty()) {
        line.push_str(" — ");
        line.push_str(note);
    }
    line
}

/// Extract the question text from an open B1 line: `- [YYYY-MM] {question}`.
/// Returns `None` for lines that don't match the open-line shape (e.g. already
/// resolved lines, or non-B1 lines).
fn open_line_question(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("- [")?;
    let (bracket, after_bracket) = rest.split_once(']')?;
    if bracket.contains('\u{2192}') {
        // contains '→' -> this is a resolved line, not an open one
        return None;
    }
    let question = after_bracket.trim_start();
    Some(question.strip_prefix('\u{2713}').unwrap_or(question).trim())
}

/// Rewrite `content` by replacing the first open line whose question matches
/// `item.question` with the resolved B1 line. If no match is found, the
/// resolved line is appended. Returns the new file content.
fn apply_resolved_line(content: &str, item: &PendingItem) -> String {
    let resolved_line = build_resolved_line(item);
    let mut replaced = false;
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

    for line in lines.iter_mut() {
        if replaced {
            continue;
        }
        if let Some(question) = open_line_question(line) {
            if question == item.question {
                *line = resolved_line.clone();
                replaced = true;
            }
        }
    }

    if !replaced {
        if !lines.is_empty() && lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        // drop trailing empty line before appending to avoid double blank
        while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            lines.pop();
        }
        lines.push(resolved_line);
    }

    let mut result = lines.join("\n");
    result.push('\n');
    result
}

/// Sync the person-level `_notes.md` file after a pending item has been
/// resolved in the database. Creates the file/parent directories if missing.
fn sync_notes_file(path: &Path, item: &PendingItem) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let existing = if path.is_file() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let updated = apply_resolved_line(&existing, item);
    std::fs::write(path, updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(question: &str, raised: &str, resolved: &str, note: Option<&str>) -> PendingItem {
        PendingItem {
            id: 1,
            person_id: 1,
            project_id: 1,
            project_name: "game-backend".to_string(),
            report_id: None,
            question: question.to_string(),
            status: "resolved".to_string(),
            raised_date: raised.to_string(),
            resolved_date: Some(resolved.to_string()),
            resolution_note: note.map(str::to_string),
        }
    }

    #[test]
    fn matching_open_line_is_rewritten() {
        let content = "- [2026-07] Why choose A?\n";
        let item = sample_item("Why choose A?", "2026-07", "2026-08", None);
        let updated = apply_resolved_line(content, &item);
        assert_eq!(updated, "- [2026-07\u{2192}2026-08] \u{2713} Why choose A?\n");
    }

    #[test]
    fn matching_open_line_with_note_is_rewritten() {
        let content = "- [2026-07] Why choose A?\n";
        let item = sample_item("Why choose A?", "2026-07", "2026-08", Some("Chose B"));
        let updated = apply_resolved_line(content, &item);
        assert_eq!(
            updated,
            "- [2026-07\u{2192}2026-08] \u{2713} Why choose A? \u{2014} Chose B\n"
        );
    }

    #[test]
    fn missing_matching_line_appends_resolved_entry() {
        let content = "- [2026-06] Some other question?\n";
        let item = sample_item("Why choose A?", "2026-07", "2026-08", None);
        let updated = apply_resolved_line(content, &item);
        assert!(updated.contains("- [2026-06] Some other question?"));
        assert!(updated.contains("- [2026-07\u{2192}2026-08] \u{2713} Why choose A?"));
    }

    #[test]
    fn empty_content_appends_resolved_entry() {
        let item = sample_item("Why choose A?", "2026-07", "2026-08", None);
        let updated = apply_resolved_line("", &item);
        assert_eq!(updated, "- [2026-07\u{2192}2026-08] \u{2713} Why choose A?\n");
    }

    #[test]
    fn sync_notes_file_creates_missing_file_and_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("nested").join("_notes.md");
        let item = sample_item("Why choose A?", "2026-07", "2026-08", None);

        sync_notes_file(&path, &item).expect("sync");

        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("Why choose A?"));
    }
}
