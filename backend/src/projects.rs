use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
struct ProjectsFile {
    projects: Vec<ProjectEntry>,
}

#[derive(Debug, Deserialize)]
struct ProjectEntry {
    name: String,
    repo_path: String,
    git_remote_url: Option<String>,
    #[serde(default)]
    default_branches: Vec<String>,
}

/// A project entry after `repo_path` resolution, ready for provisioning.
#[derive(Debug, Clone)]
pub struct ResolvedProject {
    pub name: String,
    pub repo_path: PathBuf,
    pub git_remote_url: Option<String>,
    pub default_branches: Vec<String>,
}

/// Load and upsert project rows, returning the resolved entries for provisioning.
///
/// Rows start with `is_git_repo=0`; provisioning (see [`crate::worktree`]) flips
/// it and finalizes health. Static failures — missing `git_remote_url` or empty
/// `default_branches` — are recorded as unhealthy here so provisioning can skip
/// them without a clone attempt.
pub async fn load_from_yaml(
    pool: &SqlitePool,
    data_dir: &Path,
    config_path: &Path,
) -> Result<Vec<ResolvedProject>> {
    if !config_path.exists() {
        return Err(Error::ProjectsConfigNotFound(
            config_path.display().to_string(),
        ));
    }

    let content = std::fs::read_to_string(config_path)?;
    let file: ProjectsFile = serde_yaml::from_str(&content)?;

    let mut resolved = Vec::with_capacity(file.projects.len());
    for entry in file.projects {
        let repo_path = resolve_repo_path(data_dir, &entry.repo_path);
        let repo_path_str = repo_path.display().to_string();

        let (health, health_reason) = if entry.git_remote_url.is_none() {
            ("unhealthy", Some("missing git_remote_url"))
        } else if entry.default_branches.is_empty() {
            ("unhealthy", Some("missing default_branches"))
        } else {
            ("healthy", None)
        };
        let default_branch = entry.default_branches.first().cloned();

        sqlx::query(
            r#"
            INSERT INTO projects (
                name, repo_path, git_remote_url, is_git_repo, default_branch,
                health, health_reason, updated_at
            )
            VALUES (?, ?, ?, 0, ?, ?, ?, datetime('now'))
            ON CONFLICT(name) DO UPDATE SET
                repo_path = excluded.repo_path,
                git_remote_url = excluded.git_remote_url,
                is_git_repo = 0,
                default_branch = excluded.default_branch,
                health = excluded.health,
                health_reason = excluded.health_reason,
                updated_at = datetime('now')
            "#,
        )
        .bind(&entry.name)
        .bind(&repo_path_str)
        .bind(&entry.git_remote_url)
        .bind(&default_branch)
        .bind(health)
        .bind(health_reason)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

        resolved.push(ResolvedProject {
            name: entry.name,
            repo_path,
            git_remote_url: entry.git_remote_url,
            default_branches: entry.default_branches,
        });
    }

    Ok(resolved)
}

/// Finalize a project's provisioning outcome: `is_git_repo`, `default_branch`,
/// and health. Called by [`crate::worktree::provision_all`].
pub async fn set_project_health(
    pool: &SqlitePool,
    name: &str,
    is_git_repo: i64,
    default_branch: Option<&str>,
    health: &str,
    health_reason: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE projects
         SET is_git_repo = ?, default_branch = ?, health = ?, health_reason = ?,
             updated_at = datetime('now')
         WHERE name = ?",
    )
    .bind(is_git_repo)
    .bind(default_branch)
    .bind(health)
    .bind(health_reason)
    .bind(name)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

pub async fn get_project_health(
    pool: &SqlitePool,
    name: &str,
) -> Result<(String, Option<String>)> {
    let row = sqlx::query("SELECT health, health_reason FROM projects WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok((row.get(0), row.get(1)))
}

/// Resolves `repo_path` from projects.yaml before persistence.
///
/// - Absolute paths are unchanged.
/// - Relative paths starting with `.` or `..` are unchanged (cwd-relative).
/// - Other relative values are treated as repo slugs under `{data_dir}/repos/`.
pub fn resolve_repo_path(data_dir: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        return path.to_path_buf();
    }

    match path.components().next() {
        Some(Component::CurDir) | Some(Component::ParentDir) => path.to_path_buf(),
        _ => data_dir.join("repos").join(path),
    }
}

pub async fn count_projects(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) FROM projects")
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(row.get(0))
}

pub async fn get_project(pool: &SqlitePool, name: &str) -> Result<(i64, Option<String>)> {
    let row = sqlx::query("SELECT is_git_repo, default_branch FROM projects WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    let is_git_repo: i64 = row.get(0);
    let default_branch: Option<String> = row.get(1);
    Ok((is_git_repo, default_branch))
}

pub async fn get_project_repo_path(pool: &SqlitePool, name: &str) -> Result<String> {
    sqlx::query_scalar("SELECT repo_path FROM projects WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_repo_path_slug() {
        let data_dir = Path::new("/data/reviewer");
        let resolved = resolve_repo_path(data_dir, "test/projectA");
        assert_eq!(resolved, PathBuf::from("/data/reviewer/repos/test/projectA"));
    }

    #[test]
    fn resolve_repo_path_absolute() {
        let data_dir = Path::new("/data/reviewer");
        let resolved = resolve_repo_path(data_dir, "/srv/git/projectA");
        assert_eq!(resolved, PathBuf::from("/srv/git/projectA"));
    }

    #[test]
    fn resolve_repo_path_explicit_relative() {
        let data_dir = Path::new("/data/reviewer");
        let resolved = resolve_repo_path(data_dir, "./custom/repos/projectA");
        assert_eq!(resolved, PathBuf::from("./custom/repos/projectA"));
    }
}
