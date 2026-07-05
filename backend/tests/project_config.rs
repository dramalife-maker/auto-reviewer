use reviewer_server::db::init_pool;
use reviewer_server::projects::{count_projects, get_project, load_from_yaml};
use std::fs;
use std::process::Command;

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
    load_from_yaml(&pool, &yaml_path)
        .await
        .expect("load projects yaml");

    assert_eq!(count_projects(&pool).await.expect("count"), 2);
}

#[tokio::test]
async fn git_detection_sets_default_branch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo_path = temp.path().join("sample-repo");
    fs::create_dir_all(&repo_path).expect("repo dir");

    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .arg(&repo_path)
        .output()
        .expect("git init");
    assert!(init.status.success(), "git init failed");

    let repo_path_str = repo_path.to_str().expect("repo path utf8");
    for (key, value) in [("user.email", "test@example.com"), ("user.name", "Test")] {
        let config = Command::new("git")
            .args(["-C", repo_path_str, "config", key, value])
            .output()
            .expect("git config");
        assert!(config.status.success(), "git config {key} failed");
    }

    let commit = Command::new("git")
        .args(["-C", repo_path_str, "commit", "--allow-empty", "-m", "init"])
        .output()
        .expect("git commit");
    assert!(commit.status.success(), "git commit failed");

    let yaml_path = temp.path().join("projects.yaml");
    let repo_path_display = repo_path.display().to_string().replace('\\', "/");
    fs::write(
        &yaml_path,
        format!(
            r#"projects:
  - name: sample
    repo_path: {repo_path_display}
"#
        ),
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    load_from_yaml(&pool, &yaml_path)
        .await
        .expect("load projects yaml");

    let (is_git_repo, default_branch) = get_project(&pool, "sample")
        .await
        .expect("get project");
    assert_eq!(is_git_repo, 1);
    assert_eq!(default_branch.as_deref(), Some("main"));
}
