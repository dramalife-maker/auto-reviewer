use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::runs::{self, DEFAULT_MR_REVIEW_SKIP_LABELS};

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
    #[serde(default)]
    mr_review_skip_labels: Option<Vec<String>>,
    mr_review_require_label: Option<String>,
}

/// A project entry after `repo_path` resolution, ready for provisioning.
#[derive(Debug, Clone)]
pub struct ResolvedProject {
    pub name: String,
    pub repo_path: PathBuf,
    pub git_remote_url: Option<String>,
    pub default_branches: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub source_type: String,
    pub repo_path: String,
    pub git_remote_url: Option<String>,
    #[serde(default)]
    pub default_branches: Vec<String>,
    #[serde(default)]
    pub mr_review_skip_labels: Option<Vec<String>>,
    pub mr_review_require_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectUpdateInput {
    pub source_type: String,
    pub repo_path: String,
    pub git_remote_url: Option<String>,
    #[serde(default)]
    pub default_branches: Vec<String>,
    #[serde(default)]
    pub mr_review_skip_labels: Option<Vec<String>>,
    pub mr_review_require_label: Option<String>,
}

/// Import YAML when the DB has no projects yet; otherwise SQLite is the source of truth.
pub async fn ensure_projects_loaded(
    pool: &SqlitePool,
    data_dir: &Path,
    config_path: &Path,
) -> Result<()> {
    let count = count_projects(pool).await?;
    if count > 0 {
        tracing::info!(project_count = count, "using projects from sqlite");
        return Ok(());
    }

    if !config_path.exists() {
        tracing::info!("no projects in sqlite and no projects.yaml; starting empty");
        return Ok(());
    }

    tracing::info!(config = %config_path.display(), "importing initial projects from yaml");
    load_from_yaml(pool, data_dir, config_path).await?;
    Ok(())
}

/// Load all projects from SQLite for worktree provisioning.
pub async fn load_resolved_from_db(pool: &SqlitePool) -> Result<Vec<ResolvedProject>> {
    let rows = sqlx::query(
        r#"
        SELECT name, repo_path, git_remote_url, default_branches, default_branch
        FROM projects
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let name: String = row.get(0);
            let repo_path: String = row.get(1);
            let git_remote_url: Option<String> = row.get(2);
            let default_branches_json: Option<String> = row.get(3);
            let default_branch: Option<String> = row.get(4);
            ResolvedProject {
                name,
                repo_path: PathBuf::from(repo_path),
                git_remote_url,
                default_branches: parse_default_branches(
                    default_branches_json.as_deref(),
                    default_branch.as_deref(),
                ),
            }
        })
        .collect())
}

pub async fn create_project(
    pool: &SqlitePool,
    data_dir: &Path,
    input: ProjectInput,
) -> Result<ProjectListItem> {
    validate_project_name(&input.name)?;
    let normalized = normalize_project_input(data_dir, &input)?;
    if project_exists(pool, &normalized.name).await? {
        return Err(Error::DuplicateProjectName);
    }

    upsert_project_row(pool, &normalized).await?;
    get_project_detail(pool, data_dir, &normalized.name)
        .await?
        .ok_or(Error::NotFound)
}

pub async fn update_project(
    pool: &SqlitePool,
    data_dir: &Path,
    name: &str,
    input: ProjectUpdateInput,
) -> Result<ProjectListItem> {
    if !project_exists(pool, name).await? {
        return Err(Error::NotFound);
    }

    let normalized = normalize_project_update(data_dir, name, &input)?;
    upsert_project_row(pool, &normalized).await?;
    get_project_detail(pool, data_dir, name)
        .await?
        .ok_or(Error::NotFound)
}

pub async fn delete_project(pool: &SqlitePool, name: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM projects WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    if result.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    Ok(())
}

/// Load and upsert project rows from YAML, returning the resolved entries for provisioning.
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

    tracing::info!(
        config = %config_path.display(),
        project_count = file.projects.len(),
        "loaded projects.yaml"
    );

    let mut resolved = Vec::with_capacity(file.projects.len());
    for entry in file.projects {
        let source_type = source_type_for_remote(entry.git_remote_url.as_deref());
        let mut default_branches = entry.default_branches;
        if default_branches.is_empty() && source_type == "gitlab" {
            default_branches = vec!["main".to_string()];
        }
        let normalized = normalize_project_input(
            data_dir,
            &ProjectInput {
                name: entry.name.clone(),
                source_type,
                repo_path: entry.repo_path.clone(),
                git_remote_url: entry.git_remote_url.clone(),
                default_branches,
                mr_review_skip_labels: entry.mr_review_skip_labels.clone(),
                mr_review_require_label: entry.mr_review_require_label.clone(),
            },
        )?;
        upsert_project_row(pool, &normalized).await?;
        resolved.push(ResolvedProject {
            name: entry.name,
            repo_path: normalized.repo_path,
            git_remote_url: normalized.git_remote_url,
            default_branches: normalized.default_branches,
        });
    }

    Ok(resolved)
}

#[derive(Debug, Clone)]
struct NormalizedProject {
    name: String,
    source_type: String,
    repo_path: PathBuf,
    repo_path_str: String,
    git_remote_url: Option<String>,
    default_branches: Vec<String>,
    mr_review_skip_labels: String,
    mr_review_require_label: Option<String>,
    health: &'static str,
    health_reason: Option<&'static str>,
}

async fn upsert_project_row(pool: &SqlitePool, project: &NormalizedProject) -> Result<()> {
    let default_branch = project.default_branches.first().cloned();
    let default_branches_json = serialize_default_branches(&project.default_branches);

    tracing::info!(
        name = %project.name,
        repo_path = %project.repo_path_str,
        source_type = %project.source_type,
        git_remote_url = ?project.git_remote_url,
        default_branches = ?project.default_branches,
        health = project.health,
        health_reason = ?project.health_reason,
        "project upserted"
    );

    sqlx::query(
        r#"
        INSERT INTO projects (
            name, repo_path, git_remote_url, is_git_repo, default_branch,
            health, health_reason, source_type, default_branches,
            mr_review_skip_labels, mr_review_require_label,
            updated_at
        )
        VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(name) DO UPDATE SET
            repo_path = excluded.repo_path,
            git_remote_url = excluded.git_remote_url,
            is_git_repo = 0,
            default_branch = excluded.default_branch,
            health = excluded.health,
            health_reason = excluded.health_reason,
            source_type = excluded.source_type,
            default_branches = excluded.default_branches,
            mr_review_skip_labels = excluded.mr_review_skip_labels,
            mr_review_require_label = excluded.mr_review_require_label,
            updated_at = datetime('now')
        "#,
    )
    .bind(&project.name)
    .bind(&project.repo_path_str)
    .bind(&project.git_remote_url)
    .bind(&default_branch)
    .bind(project.health)
    .bind(project.health_reason)
    .bind(&project.source_type)
    .bind(&default_branches_json)
    .bind(&project.mr_review_skip_labels)
    .bind(&project.mr_review_require_label)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    Ok(())
}

fn normalize_project_input(data_dir: &Path, input: &ProjectInput) -> Result<NormalizedProject> {
    validate_project_name(&input.name)?;
    let source_type = normalize_source_type(&input.source_type)?;
    let repo_path = resolve_repo_path(data_dir, input.repo_path.trim());
    let repo_path_str = repo_path.display().to_string();
    let git_remote_url = normalize_optional_string(input.git_remote_url.as_deref());
    let default_branches = normalize_default_branches(&input.default_branches);

    let (git_remote_url, default_branches, health, health_reason) = match source_type.as_str() {
        "gitlab" => {
            if git_remote_url.is_none() {
                return Err(Error::InvalidProjectConfig(
                    "gitlab projects require git_remote_url".into(),
                ));
            }
            if default_branches.is_empty() {
                return Err(Error::InvalidProjectConfig(
                    "gitlab projects require at least one default branch".into(),
                ));
            }
            (git_remote_url, default_branches, "healthy", None)
        }
        "local" => (None, Vec::new(), "healthy", None),
        _ => unreachable!("source_type validated"),
    };

    Ok(NormalizedProject {
        name: input.name.trim().to_string(),
        source_type,
        repo_path,
        repo_path_str,
        git_remote_url,
        default_branches,
        mr_review_skip_labels: serialize_mr_review_skip_labels(input.mr_review_skip_labels.as_ref()),
        mr_review_require_label: normalize_optional_string(input.mr_review_require_label.as_deref()),
        health,
        health_reason,
    })
}

fn serialize_mr_review_skip_labels(labels: Option<&Vec<String>>) -> String {
    match labels {
        Some(values) => {
            let normalized = normalize_default_branches(values);
            serde_json::to_string(&normalized)
                .unwrap_or_else(|_| default_mr_review_skip_labels_json())
        }
        None => default_mr_review_skip_labels_json(),
    }
}

pub fn default_mr_review_skip_labels_json() -> String {
    serde_json::to_string(
        &DEFAULT_MR_REVIEW_SKIP_LABELS
            .iter()
            .map(|label| (*label).to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| r#"["wip","do-not-review","no-ai-review"]"#.to_string())
}

fn normalize_project_update(
    data_dir: &Path,
    name: &str,
    input: &ProjectUpdateInput,
) -> Result<NormalizedProject> {
    normalize_project_input(
        data_dir,
        &ProjectInput {
            name: name.to_string(),
            source_type: input.source_type.clone(),
            repo_path: input.repo_path.clone(),
            git_remote_url: input.git_remote_url.clone(),
            default_branches: input.default_branches.clone(),
            mr_review_skip_labels: input.mr_review_skip_labels.clone(),
            mr_review_require_label: input.mr_review_require_label.clone(),
        },
    )
}

async fn project_exists(pool: &SqlitePool, name: &str) -> Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(count > 0)
}

async fn get_project_detail(
    pool: &SqlitePool,
    data_dir: &Path,
    name: &str,
) -> Result<Option<ProjectListItem>> {
    let response = list_project_details(pool, data_dir).await?;
    Ok(response.projects.into_iter().find(|project| project.name == name))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectHealth {
    pub name: String,
    pub health: String,
    pub health_reason: Option<String>,
    pub is_git_repo: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ProjectRow {
    id: i64,
    name: String,
    repo_path: String,
    git_remote_url: Option<String>,
    default_branch: Option<String>,
    default_branches: Option<String>,
    health: String,
    health_reason: Option<String>,
    is_git_repo: i64,
    source_type: String,
    mr_review_skip_labels: String,
    mr_review_require_label: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ProjectEngineerRow {
    display_name: String,
    gitlab_username: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectEngineer {
    pub display_name: String,
    pub gitlab_username: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectListItem {
    pub id: i64,
    pub name: String,
    pub repo_path: String,
    pub git_remote_url: Option<String>,
    pub default_branch: Option<String>,
    pub default_branches: Vec<String>,
    pub mr_review_skip_labels: Vec<String>,
    pub mr_review_require_label: Option<String>,
    pub health: String,
    pub health_reason: Option<String>,
    pub is_git_repo: i64,
    pub source_type: String,
    pub last_report_date: Option<String>,
    pub engineers: Vec<ProjectEngineer>,
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectListItem>,
}

/// List all projects with their current provisioning health, for the reload UI.
pub async fn list_projects(pool: &SqlitePool) -> Result<Vec<ProjectHealth>> {
    sqlx::query_as::<_, ProjectHealth>(
        "SELECT name, health, health_reason, is_git_repo FROM projects ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

/// List projects with detail fields for the settings UI.
pub async fn list_project_details(pool: &SqlitePool, data_dir: &Path) -> Result<ProjectListResponse> {
    let rows = sqlx::query_as::<_, ProjectRow>(
        r#"
        SELECT id, name, repo_path, git_remote_url, default_branch, default_branches,
               health, health_reason, is_git_repo, source_type,
               mr_review_skip_labels, mr_review_require_label
        FROM projects
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    let last_report_dates = list_last_report_dates(pool).await?;
    let mut projects = Vec::with_capacity(rows.len());
    for row in rows {
        let default_branches =
            parse_default_branches(row.default_branches.as_deref(), row.default_branch.as_deref());
        let engineers = list_project_engineers(pool, row.id).await?;
        projects.push(ProjectListItem {
            id: row.id,
            name: row.name,
            repo_path: display_repo_path(data_dir, &row.repo_path),
            git_remote_url: row.git_remote_url,
            default_branch: row.default_branch,
            default_branches,
            mr_review_skip_labels: runs::parse_mr_review_skip_labels(&row.mr_review_skip_labels),
            mr_review_require_label: row.mr_review_require_label,
            health: row.health,
            health_reason: row.health_reason,
            is_git_repo: row.is_git_repo,
            source_type: row.source_type,
            last_report_date: last_report_dates.get(&row.id).cloned(),
            engineers,
        });
    }

    Ok(ProjectListResponse { projects })
}

#[derive(Debug, sqlx::FromRow)]
struct LastReportDateRow {
    project_id: i64,
    report_date: String,
}

async fn list_last_report_dates(pool: &SqlitePool) -> Result<std::collections::HashMap<i64, String>> {
    let rows = sqlx::query_as::<_, LastReportDateRow>(
        "SELECT project_id, MAX(report_date) AS report_date FROM reports GROUP BY project_id",
    )
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    Ok(rows
        .into_iter()
        .map(|row| (row.project_id, row.report_date))
        .collect())
}

async fn list_project_engineers(pool: &SqlitePool, project_id: i64) -> Result<Vec<ProjectEngineer>> {
    let rows = sqlx::query_as::<_, ProjectEngineerRow>(
        r#"
        SELECT DISTINCT p.display_name,
               (
                   SELECT pi.value
                   FROM person_identities pi
                   WHERE pi.person_id = p.id
                     AND pi.kind IN ('gitlab_user', 'glab_user')
                   ORDER BY pi.id
                   LIMIT 1
               ) AS gitlab_username
        FROM (
            SELECT person_id FROM participation WHERE project_id = ?
            UNION
            SELECT person_id FROM reports WHERE project_id = ?
        ) participants
        JOIN people p ON p.id = participants.person_id
        ORDER BY p.display_name
        "#,
    )
    .bind(project_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    Ok(rows
        .into_iter()
        .map(|row| ProjectEngineer {
            display_name: row.display_name,
            gitlab_username: row.gitlab_username,
        })
        .collect())
}

fn source_type_for_remote(git_remote_url: Option<&str>) -> String {
    match git_remote_url.map(str::trim).filter(|value| !value.is_empty()) {
        Some(_) => "gitlab".to_string(),
        None => "local".to_string(),
    }
}

fn normalize_source_type(source_type: &str) -> Result<String> {
    match source_type.trim().to_ascii_lowercase().as_str() {
        "gitlab" => Ok("gitlab".to_string()),
        "local" => Ok("local".to_string()),
        _ => Err(Error::InvalidProjectConfig(format!(
            "unsupported source_type: {source_type}"
        ))),
    }
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_default_branches(branches: &[String]) -> Vec<String> {
    branches
        .iter()
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
        .collect()
}

fn serialize_default_branches(branches: &[String]) -> Option<String> {
    if branches.is_empty() {
        return None;
    }
    serde_json::to_string(branches).ok()
}

fn parse_default_branches(json: Option<&str>, fallback_branch: Option<&str>) -> Vec<String> {
    if let Some(json) = json {
        if let Ok(branches) = serde_json::from_str::<Vec<String>>(json) {
            let normalized = normalize_default_branches(&branches);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }
    fallback_branch
        .map(|branch| vec![branch.to_string()])
        .unwrap_or_default()
}

fn validate_project_name(name: &str) -> Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 120 {
        return Err(Error::InvalidProjectName);
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(Error::InvalidProjectName);
    }
    Ok(())
}

/// Show slug form in the UI when the stored path is the default repos layout.
fn display_repo_path(data_dir: &Path, stored: &str) -> String {
    let stored_path = Path::new(stored);
    let repos_root = data_dir.join("repos");
    if let Ok(suffix) = stored_path.strip_prefix(&repos_root) {
        return suffix.display().to_string();
    }
    stored.to_string()
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

/// Resolves `repo_path` before persistence.
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

    #[test]
    fn display_repo_path_shows_slug() {
        let data_dir = Path::new("/data/reviewer");
        assert_eq!(
            display_repo_path(data_dir, "/data/reviewer/repos/game-backend"),
            "game-backend"
        );
    }

    #[test]
    fn parse_default_branches_from_json() {
        assert_eq!(
            parse_default_branches(Some(r#"["main","develop"]"#), None),
            vec!["main".to_string(), "develop".to_string()]
        );
    }
}
