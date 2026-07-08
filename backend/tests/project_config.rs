use reviewer_server::db::init_pool;
use reviewer_server::projects::{
    count_projects, get_project, get_project_health, get_project_repo_path, load_from_yaml,
};
use reviewer_server::worktree::provision_all;
use std::fs;
use std::process::Command;

fn normalize_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Create a local non-bare git repo with one commit on `main`, usable as a
/// clone source (a local path is a valid `git_remote_url`).
fn init_source_repo(path: &std::path::Path) {
    fs::create_dir_all(path).expect("repo dir");
    let path_str = path.to_str().expect("repo path utf8");
    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .arg(path)
        .output()
        .expect("git init");
    assert!(init.status.success(), "git init failed");
    for (key, value) in [("user.email", "test@example.com"), ("user.name", "Test")] {
        let config = Command::new("git")
            .args(["-C", path_str, "config", key, value])
            .output()
            .expect("git config");
        assert!(config.status.success(), "git config {key} failed");
    }
    let commit = Command::new("git")
        .args(["-C", path_str, "commit", "--allow-empty", "-m", "init"])
        .output()
        .expect("git commit");
    assert!(commit.status.success(), "git commit failed");
}

#[tokio::test]
async fn projects_yaml_loads_two_rows() {
    let temp = tempfile::tempdir().expect("tempdir");
    let yaml_path = temp.path().join("projects.yaml");
    fs::write(
        &yaml_path,
        r#"projects:
  - name: game-backend
    repo_path: /data/reviewer/repos/game-backend
  - name: web-portal
    repo_path: /data/reviewer/repos/web-portal
    git_remote_url: git@gitlab.example.com:team/web-portal.git
"#,
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    load_from_yaml(&pool, temp.path(), &yaml_path)
        .await
        .expect("load projects yaml");

    assert_eq!(count_projects(&pool).await.expect("count"), 2);
}

#[tokio::test]
async fn repo_slug_loads_resolved_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let yaml_path = temp.path().join("projects.yaml");
    fs::write(
        &yaml_path,
        r#"projects:
  - name: project-a
    repo_path: test/projectA
"#,
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    load_from_yaml(&pool, temp.path(), &yaml_path)
        .await
        .expect("load projects yaml");

    let stored = get_project_repo_path(&pool, "project-a")
        .await
        .expect("repo path");
    let expected = temp.path().join("repos").join("test").join("projectA");
    assert_eq!(normalize_path(std::path::Path::new(&stored)), normalize_path(&expected));
}

#[tokio::test]
async fn provisioning_sets_git_repo_and_default_branch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source-repo");
    init_source_repo(&source);
    let source_url = normalize_path(&source);

    let container = temp.path().join("repos/sample");
    let container_display = normalize_path(&container);
    let yaml_path = temp.path().join("projects.yaml");
    fs::write(
        &yaml_path,
        format!(
            r#"projects:
  - name: sample
    repo_path: {container_display}
    git_remote_url: {source_url}
    default_branches:
      - main
"#
        ),
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let resolved = load_from_yaml(&pool, temp.path(), &yaml_path)
        .await
        .expect("load projects yaml");
    provision_all(&pool, &resolved).await;

    let (is_git_repo, default_branch) = get_project(&pool, "sample").await.expect("get project");
    assert_eq!(is_git_repo, 1);
    assert_eq!(default_branch.as_deref(), Some("main"));

    let (health, _) = get_project_health(&pool, "sample").await.expect("health");
    assert_eq!(health, "healthy");
    assert!(container.join(".bare").is_dir(), ".bare provisioned");
    assert!(container.join("main").is_dir(), "resident worktree provisioned");
}

#[tokio::test]
async fn missing_remote_url_marks_local_and_isolates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source-repo");
    init_source_repo(&source);
    let source_url = normalize_path(&source);
    let good_container = normalize_path(&temp.path().join("repos/good"));
    let yaml_path = temp.path().join("projects.yaml");
    fs::write(
        &yaml_path,
        format!(
            r#"projects:
  - name: no-url
    repo_path: repos/no-url
  - name: good
    repo_path: {good_container}
    git_remote_url: {source_url}
    default_branches:
      - main
"#
        ),
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let resolved = load_from_yaml(&pool, temp.path(), &yaml_path)
        .await
        .expect("load projects yaml");
    provision_all(&pool, &resolved).await;

    let (health, reason) = get_project_health(&pool, "no-url").await.expect("health");
    assert_eq!(health, "healthy");
    assert!(reason.is_none());

    // Local projects skip provisioning; the gitlab sibling still provisions.
    let (is_git_repo, _) = get_project(&pool, "good").await.expect("get good");
    assert_eq!(is_git_repo, 1);
}
