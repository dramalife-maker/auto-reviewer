use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use reviewer_server::projects::load_from_yaml;
use reviewer_server::runs;
use reviewer_server::summary::{count_pending_for_person, count_reports_for_run, ingest_project_summaries, parse_summary_file};
use reviewer_server::worker::{process_run_project, resolve_working_dir};
use reviewer_server::worktree::provision_all;
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_app_state_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

async fn insert_projects(pool: &sqlx::SqlitePool, temp: &tempfile::TempDir) {
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(pool)
    .await
    .expect("insert alpha");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('beta', ?, 0)",
    )
    .bind(temp.path().join("repos/beta").display().to_string())
    .execute(pool)
    .await
    .expect("insert beta");
}

#[tokio::test]
async fn manual_all_run_enqueues_projects() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"trigger":"manual_all"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let run_id = json["run_id"].as_i64().expect("run_id");

    assert_eq!(
        runs::count_run_projects_by_state(&pool, run_id, "queued")
            .await
            .expect("count"),
        2
    );
}

#[tokio::test]
async fn manual_project_run_enqueues_one_project() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"trigger":"manual_project","project_name":"alpha"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let run_id = json["run_id"].as_i64().expect("run_id");

    assert_eq!(
        runs::count_run_projects_by_state(&pool, run_id, "queued")
            .await
            .expect("count"),
        1
    );

    let trigger: String = sqlx::query_scalar("SELECT trigger FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("trigger");
    assert_eq!(trigger, "manual_project");

    let project_name: String = sqlx::query_scalar(
        "SELECT p.name FROM run_projects rp
         INNER JOIN projects p ON p.id = rp.project_id
         WHERE rp.run_id = ?",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("project name");
    assert_eq!(project_name, "alpha");
}

#[tokio::test]
async fn duplicate_project_run_returns_409() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");

    let run_id = run_result.last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'running')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"trigger":"manual_all"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn worker_marks_skipped_timeout() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let slow_executor = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/slow_executor.cmd");
    std::env::set_var("REVIEWER_EXECUTOR", &slow_executor);

    let pool = init_pool(temp.path()).await.expect("init pool");
    std::fs::create_dir_all(temp.path().join("repos/alpha")).expect("repo dir");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    sqlx::query("UPDATE schedule_config SET per_project_timeout_sec = 1 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("update timeout");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_project_result = sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project");

    let run_project_id = run_project_result.last_insert_rowid();

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let job = runs::RunProjectRow {
        id: run_project_id,
        run_id,
        project_id,
        name: "alpha".into(),
        repo_path: temp.path().join("repos/alpha").display().to_string(),
    };

    process_run_project(&pool, &config, job, 1)
        .await
        .expect("process run project");

    let state: String = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(run_project_id)
        .fetch_one(&pool)
        .await
        .expect("state");

    assert_eq!(state, "skipped_timeout");

    std::env::remove_var("REVIEWER_EXECUTOR");
}

fn init_source_repo(path: &std::path::Path) {
    use std::process::Command;
    std::fs::create_dir_all(path).expect("source dir");
    let p = path.display().to_string();
    for args in [
        vec!["init", "-b", "main", &p],
        vec!["-C", &p, "config", "user.email", "t@e.com"],
        vec!["-C", &p, "config", "user.name", "T"],
        vec!["-C", &p, "commit", "--allow-empty", "-m", "init"],
    ] {
        let out = Command::new("git").args(&args).output().expect("git");
        assert!(out.status.success(), "git {args:?}");
    }
}

#[tokio::test]
async fn resolve_working_dir_returns_resident_worktree() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    std::env::remove_var("REVIEWER_EXECUTOR");
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source_repo(&source);
    let container = temp.path().join("repos/svc");
    let container_display = container.display().to_string().replace('\\', "/");
    let source_url = source.display().to_string().replace('\\', "/");

    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(
        &yaml_path,
        format!(
            "projects:\n  - name: svc\n    repo_path: {container_display}\n    git_remote_url: {source_url}\n    default_branches:\n      - main\n"
        ),
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let resolved = load_from_yaml(&pool, temp.path(), &yaml_path).await.expect("load");
    provision_all(&pool, &resolved).await;

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'svc'")
        .fetch_one(&pool)
        .await
        .expect("project id");
    let job = runs::RunProjectRow {
        id: 1,
        run_id: 1,
        project_id,
        name: "svc".into(),
        repo_path: container.display().to_string(),
    };

    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let dir = resolve_working_dir(&pool, &config, &job)
        .await
        .expect("resolve dir");
    assert_eq!(dir, container.join("main"), "resident worktree path");

    // An unhealthy / unprovisioned project cannot supply a worktree.
    let bad_job = runs::RunProjectRow {
        id: 2,
        run_id: 1,
        project_id: 999,
        name: "missing".into(),
        repo_path: container.display().to_string(),
    };
    assert!(resolve_working_dir(&pool, &config, &bad_job).await.is_err());
}

#[tokio::test]
async fn summary_parser_creates_report_and_pending() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
        .execute(&pool)
        .await
        .expect("insert person");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-05/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-05
one_line: Stable week
mr_count: 6
commit_count: 42
---

## 待確認
- First question?
- Second question?
"#,
    )
    .expect("write summary");

    let parsed = parse_summary_file(&summary_path).expect("parse summary");
    assert_eq!(parsed.pending_questions.len(), 2);

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    ingest_project_summaries(
        &pool,
        temp.path(),
        "game-backend",
        1,
        run_id,
    )
    .await
    .expect("ingest summaries");

    assert_eq!(
        count_reports_for_run(&pool, run_id).await.expect("reports"),
        1
    );
    assert_eq!(
        count_pending_for_person(&pool, "Alice")
            .await
            .expect("pending"),
        2
    );

    let one_line: String = sqlx::query_scalar(
        "SELECT one_line FROM reports WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("one_line");
    assert_eq!(one_line, "Stable week");
}
