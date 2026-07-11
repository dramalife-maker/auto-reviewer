use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::process::Command;

use crate::Error;

pub const KIND_GIT_EMAIL: &str = "git_email";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestAuthor {
    pub email: String,
    pub git_name: String,
    pub person_id: i64,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedPerson {
    pub person_id: i64,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UnmatchedAuthorItem {
    pub id: i64,
    pub kind: String,
    pub value: String,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub commit_count: i64,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct IdentityItem {
    pub id: i64,
    pub kind: String,
    pub value: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
struct GitAuthor {
    email: String,
    git_name: String,
    commit_count: i64,
}

pub fn normalize_git_email(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub async fn resolve_person_by_email(
    pool: &SqlitePool,
    email: &str,
) -> Result<Option<ResolvedPerson>, Error> {
    let normalized = normalize_git_email(email);
    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT p.id, p.display_name
         FROM person_identities pi
         INNER JOIN people p ON p.id = pi.person_id
         WHERE pi.kind = ? AND pi.value = ?",
    )
    .bind(KIND_GIT_EMAIL)
    .bind(&normalized)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    Ok(row.map(|(person_id, display_name)| ResolvedPerson {
        person_id,
        display_name,
    }))
}

pub async fn record_unmatched_author(
    pool: &SqlitePool,
    kind: &str,
    value: &str,
    project_id: i64,
    commit_count: i64,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO unmatched_authors (kind, value, project_id, commit_count, first_seen, last_seen)
         VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
         ON CONFLICT(kind, value) DO UPDATE SET
            project_id = excluded.project_id,
            commit_count = unmatched_authors.commit_count + excluded.commit_count,
            last_seen = datetime('now')",
    )
    .bind(kind)
    .bind(value)
    .bind(project_id)
    .bind(commit_count)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

pub async fn prepare_manifest_authors(
    pool: &SqlitePool,
    worktree: &Path,
    project_id: i64,
    since: &str,
    run_date: &str,
) -> Result<Vec<ManifestAuthor>, Error> {
    let authors = enumerate_git_authors(worktree, since, run_date).await?;
    let mut by_person: HashMap<i64, ManifestAuthor> = HashMap::new();

    for author in authors {
        let normalized_email = normalize_git_email(&author.email);
        match resolve_person_by_email(pool, &normalized_email).await? {
            Some(resolved) => {
                by_person
                    .entry(resolved.person_id)
                    .and_modify(|entry| {
                        if author.commit_count > 0 {
                            entry.git_name = author.git_name.clone();
                            entry.email = normalized_email.clone();
                        }
                    })
                    .or_insert(ManifestAuthor {
                        email: normalized_email,
                        git_name: author.git_name,
                        person_id: resolved.person_id,
                        display_name: resolved.display_name,
                    });
            }
            None => {
                record_unmatched_author(
                    pool,
                    KIND_GIT_EMAIL,
                    &normalized_email,
                    project_id,
                    author.commit_count,
                )
                .await?;
            }
        }
    }

    let mut manifest_authors: Vec<ManifestAuthor> = by_person.into_values().collect();
    manifest_authors.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(manifest_authors)
}

async fn enumerate_git_authors(
    worktree: &Path,
    since: &str,
    run_date: &str,
) -> Result<Vec<GitAuthor>, Error> {
    if !worktree.join(".git").exists() && !worktree.is_dir() {
        return Ok(Vec::new());
    }

    let since_arg = format!("{since}T00:00:00");
    let until_arg = format!("{run_date}T23:59:59");
    let output = Command::new("git")
        .current_dir(worktree)
        .args([
            "log",
            &format!("--since={since_arg}"),
            &format!("--until={until_arg}"),
            "--format=%ae|%an",
            "--no-merges",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(Error::Io)?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut counts: HashMap<String, (String, i64)> = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((email, git_name)) = line.split_once('|') else {
            continue;
        };
        let normalized = normalize_git_email(email);
        if normalized.is_empty() {
            continue;
        }
        let entry = counts
            .entry(normalized)
            .or_insert_with(|| (git_name.trim().to_string(), 0));
        entry.1 += 1;
    }

    Ok(counts
        .into_iter()
        .map(|(email, (git_name, commit_count))| GitAuthor {
            email,
            git_name,
            commit_count,
        })
        .collect())
}

pub async fn list_unmatched_authors(pool: &SqlitePool) -> Result<Vec<UnmatchedAuthorItem>, Error> {
    sqlx::query_as::<_, UnmatchedAuthorItem>(
        "SELECT ua.id, ua.kind, ua.value, ua.project_id, p.name AS project_name,
                ua.commit_count, ua.first_seen, ua.last_seen
         FROM unmatched_authors ua
         LEFT JOIN projects p ON p.id = ua.project_id
         ORDER BY ua.last_seen DESC, ua.value",
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

pub async fn create_person(pool: &SqlitePool, display_name: &str) -> Result<i64, Error> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(Error::InvalidPersonName);
    }

    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM people WHERE display_name = ?")
        .bind(display_name)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
    if exists.is_some() {
        return Err(Error::DuplicateDisplayName);
    }

    let result = sqlx::query("INSERT INTO people (display_name) VALUES (?)")
        .bind(display_name)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(result.last_insert_rowid())
}

pub async fn bind_identity(
    pool: &SqlitePool,
    person_id: i64,
    kind: &str,
    value: &str,
    label: Option<&str>,
) -> Result<(), Error> {
    let person_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
    if person_exists.is_none() {
        return Err(Error::NotFound);
    }

    let normalized_value = normalize_identity_value(kind, value)?;
    let existing_person: Option<i64> = sqlx::query_scalar(
        "SELECT person_id FROM person_identities WHERE kind = ? AND value = ?",
    )
    .bind(kind)
    .bind(&normalized_value)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    if let Some(existing) = existing_person {
        if existing != person_id {
            return Err(Error::IdentityConflict);
        }
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO person_identities (person_id, kind, value, label) VALUES (?, ?, ?, ?)",
    )
    .bind(person_id)
    .bind(kind)
    .bind(&normalized_value)
    .bind(label)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    sqlx::query("DELETE FROM unmatched_authors WHERE kind = ? AND value = ?")
        .bind(kind)
        .bind(&normalized_value)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

    Ok(())
}

pub async fn list_identities_for_person(
    pool: &SqlitePool,
    person_id: i64,
) -> Result<Vec<IdentityItem>, Error> {
    let person_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
    if person_exists.is_none() {
        return Err(Error::NotFound);
    }

    sqlx::query_as::<_, IdentityItem>(
        "SELECT id, kind, value, label FROM person_identities WHERE person_id = ? ORDER BY kind, value",
    )
    .bind(person_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PersonProjectItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonDetail {
    pub id: i64,
    pub display_name: String,
    pub identities: Vec<IdentityItem>,
    pub projects: Vec<PersonProjectItem>,
}

pub async fn get_person_detail(pool: &SqlitePool, person_id: i64) -> Result<PersonDetail, Error> {
    let display_name: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
            .bind(person_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;
    let Some(display_name) = display_name else {
        return Err(Error::NotFound);
    };

    let identities = list_identities_for_person(pool, person_id).await?;
    let projects = sqlx::query_as::<_, PersonProjectItem>(
        "SELECT id, name FROM projects
         WHERE id IN (
             SELECT project_id FROM reports WHERE person_id = ?
             UNION
             SELECT project_id FROM participation WHERE person_id = ?
         )
         ORDER BY name",
    )
    .bind(person_id)
    .bind(person_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    Ok(PersonDetail {
        id: person_id,
        display_name,
        identities,
        projects,
    })
}

pub async fn rename_person(
    pool: &SqlitePool,
    data_root: &Path,
    person_id: i64,
    new_display_name: &str,
) -> Result<PersonDetail, Error> {
    let new_display_name = new_display_name.trim();
    if new_display_name.is_empty() {
        return Err(Error::InvalidPersonName);
    }

    let old_display_name: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM people WHERE id = ?")
            .bind(person_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;
    let Some(old_display_name) = old_display_name else {
        return Err(Error::NotFound);
    };

    if old_display_name == new_display_name {
        return get_person_detail(pool, person_id).await;
    }

    let duplicate: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM people WHERE display_name = ? AND id != ?",
    )
    .bind(new_display_name)
    .bind(person_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;
    if duplicate.is_some() {
        return Err(Error::DuplicateDisplayName);
    }

    let old_dir = crate::person_trends::person_trends_dir(data_root, &old_display_name);
    let new_dir = crate::person_trends::person_trends_dir(data_root, new_display_name);

    if new_dir.exists() {
        return Err(Error::PeopleDirectoryConflict);
    }

    sqlx::query(
        "UPDATE people SET display_name = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(new_display_name)
    .bind(person_id)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    if old_dir.exists() {
        if let Err(err) = std::fs::rename(&old_dir, &new_dir) {
            let _ = sqlx::query(
                "UPDATE people SET display_name = ?, updated_at = datetime('now') WHERE id = ?",
            )
            .bind(&old_display_name)
            .bind(person_id)
            .execute(pool)
            .await;
            return Err(Error::Io(err));
        }
    }

    get_person_detail(pool, person_id).await
}

pub async fn unbind_identity(
    pool: &SqlitePool,
    person_id: i64,
    identity_id: i64,
) -> Result<(), Error> {
    let person_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM people WHERE id = ?")
        .bind(person_id)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;
    if person_exists.is_none() {
        return Err(Error::NotFound);
    }

    let result = sqlx::query(
        "DELETE FROM person_identities WHERE id = ? AND person_id = ?",
    )
    .bind(identity_id)
    .bind(person_id)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    if result.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    Ok(())
}

fn normalize_identity_value(kind: &str, value: &str) -> Result<String, Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidIdentityValue);
    }
    if kind == KIND_GIT_EMAIL {
        return Ok(normalize_git_email(trimmed));
    }
    Ok(trimmed.to_string())
}

pub async fn resolve_person_id_by_display_name(
    pool: &SqlitePool,
    display_name: &str,
) -> Result<Option<i64>, Error> {
    sqlx::query_scalar("SELECT id FROM people WHERE display_name = ?")
        .bind(display_name)
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)
}
