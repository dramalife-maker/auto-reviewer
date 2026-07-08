use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::SqlitePool;

use crate::Error;

pub const PERSON_REPORT_DIR: &str = "_people";

#[derive(Debug, Serialize)]
pub struct GrowthTimelineEntry {
    pub month: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PersonTrendsResponse {
    pub person_id: i64,
    pub display_name: String,
    pub long_term_observation: String,
    pub growth_timeline: Vec<GrowthTimelineEntry>,
    pub historical_pending: Vec<String>,
}

pub fn person_report_root(data_root: &Path) -> PathBuf {
    data_root.join("reports").join(PERSON_REPORT_DIR)
}

pub fn is_person_level_report_name(name: &str) -> bool {
    name == PERSON_REPORT_DIR
}

pub fn person_trends_dir(data_root: &Path, display_name: &str) -> PathBuf {
    person_report_root(data_root).join(display_name)
}

pub async fn load_trends(
    pool: &SqlitePool,
    data_root: &Path,
    person_id: i64,
) -> Result<Option<PersonTrendsResponse>, Error> {
    let display_name: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
            .bind(person_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

    let Some(display_name) = display_name else {
        return Ok(None);
    };

    let person_dir = person_trends_dir(data_root, &display_name);
    Ok(Some(PersonTrendsResponse {
        person_id,
        display_name: display_name.clone(),
        long_term_observation: read_file_or_empty(&person_dir.join("index.md")),
        growth_timeline: read_growth_timeline(&person_dir),
        historical_pending: read_historical_pending(&person_dir.join("_notes.md")),
    }))
}

fn read_file_or_empty(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn read_growth_timeline(person_dir: &Path) -> Vec<GrowthTimelineEntry> {
    let Ok(entries) = std::fs::read_dir(person_dir) else {
        return Vec::new();
    };

    let mut months: Vec<(String, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let file_name = path.file_name()?.to_str()?;
            if is_monthly_trends_file(file_name) {
                Some((file_name.trim_end_matches(".md").to_string(), path))
            } else {
                None
            }
        })
        .collect();

    months.sort_by(|a, b| b.0.cmp(&a.0));

    months
        .into_iter()
        .map(|(month, path)| GrowthTimelineEntry {
            month,
            content: read_file_or_empty(&path),
        })
        .collect()
}

fn is_monthly_trends_file(file_name: &str) -> bool {
    let Some(stem) = file_name.strip_suffix(".md") else {
        return false;
    };
    let mut parts = stem.split('-');
    let Some(year) = parts.next() else {
        return false;
    };
    let Some(month) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    year.len() == 4
        && year.chars().all(|ch| ch.is_ascii_digit())
        && month.len() == 2
        && month.chars().all(|ch| ch.is_ascii_digit())
}

fn read_historical_pending(path: &Path) -> Vec<String> {
    let content = read_file_or_empty(path);
    content
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- ["))
        .map(str::to_string)
        .collect()
}
