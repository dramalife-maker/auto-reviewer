//! Global review file ignore list (single row, `id = 1`).
//!
//! Stores raw git pathspec patterns. The `:(exclude)` magic prefix is applied
//! at call time by whoever builds the git command — never stored — so a stored
//! value can never smuggle a second magic prefix into the argument list.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::{Error, Result};

/// Per-entry length cap. Long enough for a deep path glob, short enough that a
/// pasted file or accidental blob is rejected instead of reaching git.
pub const MAX_GLOB_LEN: usize = 200;

/// List size cap. Keeps the pathspec argument list bounded.
pub const MAX_GLOB_COUNT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSettings {
    pub ignore_globs: Vec<String>,
}

/// Trim, drop blanks, and dedupe while keeping first-occurrence order, then
/// enforce the rejection rules. Normalization is silent; only the rules below
/// are errors.
pub fn normalize_ignore_globs(input: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::with_capacity(input.len());
    for raw in input {
        let value = raw.trim();
        if value.is_empty() {
            continue;
        }
        if value.starts_with(':') {
            return Err(Error::InvalidReviewSettings(format!(
                "ignore glob must not start with ':' (pathspec magic is added by the server): {value}"
            )));
        }
        if value.chars().count() > MAX_GLOB_LEN {
            return Err(Error::InvalidReviewSettings(format!(
                "ignore glob exceeds {MAX_GLOB_LEN} characters"
            )));
        }
        if !out.iter().any(|existing| existing == value) {
            out.push(value.to_string());
        }
    }
    if out.len() > MAX_GLOB_COUNT {
        return Err(Error::InvalidReviewSettings(format!(
            "ignore list exceeds {MAX_GLOB_COUNT} entries"
        )));
    }
    Ok(out)
}

/// Wrap stored patterns as git exclude pathspecs.
///
/// No `:(glob)` magic: without it a `*` crosses directory separators, so
/// `*.lock` matches `frontend/pnpm-lock.yaml` the way a user filling in an
/// extension rule expects.
pub fn to_exclude_pathspecs(globs: &[String]) -> Vec<String> {
    globs
        .iter()
        .map(|glob| format!(":(exclude){glob}"))
        .collect()
}

pub async fn load(pool: &SqlitePool) -> Result<ReviewSettings> {
    let raw: String = sqlx::query_scalar("SELECT ignore_globs FROM review_settings WHERE id = 1")
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(ReviewSettings {
        ignore_globs: parse_ignore_globs(&raw),
    })
}

/// Full replacement. The normalized list is what gets stored and returned.
pub async fn replace(pool: &SqlitePool, input: &[String]) -> Result<ReviewSettings> {
    let normalized = normalize_ignore_globs(input)?;
    let json = serde_json::to_string(&normalized)
        .map_err(|err| Error::InvalidReviewSettings(format!("serialize ignore globs: {err}")))?;
    sqlx::query("UPDATE review_settings SET ignore_globs = ?, updated_at = datetime('now') WHERE id = 1")
        .bind(&json)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(ReviewSettings {
        ignore_globs: normalized,
    })
}

/// A malformed stored value degrades to "no filtering" rather than failing the
/// run — the same posture as the pathspec fallback in change materials.
fn parse_ignore_globs(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let out = normalize_ignore_globs(&globs(&["  *.lock  "])).expect("normalize");
        assert_eq!(out, globs(&["*.lock"]));
    }

    #[test]
    fn drops_duplicates_keeping_first_occurrence() {
        let out = normalize_ignore_globs(&globs(&["*.lock", "*.lock"])).expect("normalize");
        assert_eq!(out, globs(&["*.lock"]));
    }

    #[test]
    fn discards_blank_entries() {
        let out = normalize_ignore_globs(&globs(&["*.lock", "", "   "])).expect("normalize");
        assert_eq!(out, globs(&["*.lock"]));
    }

    #[test]
    fn preserves_input_order() {
        let out = normalize_ignore_globs(&globs(&["b.lock", "a.lock"])).expect("normalize");
        assert_eq!(out, globs(&["b.lock", "a.lock"]));
    }

    #[test]
    fn rejects_leading_colon() {
        for entry in [":(exclude)*.lock", ":(top)"] {
            let err = normalize_ignore_globs(&globs(&[entry])).expect_err("must reject");
            assert!(
                matches!(err, Error::InvalidReviewSettings(_)),
                "unexpected error for {entry}: {err}"
            );
        }
    }

    #[test]
    fn rejects_overlong_entry() {
        let long = "a".repeat(MAX_GLOB_LEN + 1);
        let err = normalize_ignore_globs(&globs(&[long.as_str()])).expect_err("must reject");
        assert!(matches!(err, Error::InvalidReviewSettings(_)));

        let at_limit = "a".repeat(MAX_GLOB_LEN);
        normalize_ignore_globs(&globs(&[at_limit.as_str()])).expect("limit itself is allowed");
    }

    #[test]
    fn rejects_too_many_entries() {
        let too_many: Vec<String> = (0..=MAX_GLOB_COUNT).map(|i| format!("f{i}.lock")).collect();
        let err = normalize_ignore_globs(&too_many).expect_err("must reject");
        assert!(matches!(err, Error::InvalidReviewSettings(_)));

        let at_limit: Vec<String> = (0..MAX_GLOB_COUNT).map(|i| format!("f{i}.lock")).collect();
        normalize_ignore_globs(&at_limit).expect("limit itself is allowed");
    }

    #[test]
    fn builds_exclude_pathspecs_without_glob_magic() {
        let out = to_exclude_pathspecs(&globs(&["*.lock", "vendor/**"]));
        assert_eq!(out, globs(&[":(exclude)*.lock", ":(exclude)vendor/**"]));
    }

    #[test]
    fn empty_list_produces_no_pathspecs() {
        assert!(to_exclude_pathspecs(&[]).is_empty());
    }
}
