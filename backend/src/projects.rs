use std::path::{Component, Path, PathBuf};
use std::process::Command;

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
}

pub async fn load_from_yaml(
    pool: &SqlitePool,
    data_dir: &Path,
    config_path: &Path,
) -> Result<()> {
    if !config_path.exists() {
        return Err(Error::ProjectsConfigNotFound(
            config_path.display().to_string(),
        ));
    }

    let content = std::fs::read_to_string(config_path)?;
    let file: ProjectsFile = serde_yaml::from_str(&content)?;

    for entry in file.projects {
        let repo_path = resolve_repo_path(data_dir, &entry.repo_path);
        let repo_path_str = repo_path.display().to_string();
        let (is_git_repo, default_branch) = detect_git(&repo_path);

        sqlx::query(
            r#"
            INSERT INTO projects (
                name, repo_path, git_remote_url, is_git_repo, default_branch, updated_at
            )
            VALUES (?, ?, ?, ?, ?, datetime('now'))
            ON CONFLICT(name) DO UPDATE SET
                repo_path = excluded.repo_path,
                git_remote_url = excluded.git_remote_url,
                is_git_repo = excluded.is_git_repo,
                default_branch = excluded.default_branch,
                updated_at = datetime('now')
            "#,
        )
        .bind(&entry.name)
        .bind(&repo_path_str)
        .bind(&entry.git_remote_url)
        .bind(is_git_repo)
        .bind(default_branch)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    }

    Ok(())
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

fn detect_git(repo_path: &Path) -> (i64, Option<String>) {
    if !repo_path.is_dir() {
        return (0, None);
    }

    let git_dir = repo_path.join(".git");
    if !(git_dir.is_dir() || git_dir.is_file()) {
        return (0, None);
    }

    let Some(repo_str) = repo_path.to_str() else {
        return (1, None);
    };

    let output = Command::new("git")
        .args(["-C", repo_str, "rev-parse", "--abbrev-ref", "HEAD"])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let branch = String::from_utf8_lossy(&result.stdout).trim().to_string();
            if branch.is_empty() {
                (1, None)
            } else {
                (1, Some(branch))
            }
        }
        _ => (1, None),
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
