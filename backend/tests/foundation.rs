use std::process::Command;
use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::{foreign_keys_enabled, init_pool, table_exists};
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn startup_fails_without_data_dir() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_reviewer-server"))
        .current_dir(temp.path())
        .env_remove("DATA_ROOT_DIR")
        .output()
        .expect("failed to run reviewer-server binary");

    assert!(
        !output.status.success(),
        "expected non-zero exit when DATA_ROOT_DIR is unset"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DATA_ROOT_DIR"),
        "stderr should mention DATA_ROOT_DIR, got: {stderr}"
    );
}

#[tokio::test]
async fn migrations_apply_on_empty_db() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path())
        .await
        .expect("init pool on empty db");

    assert!(
        foreign_keys_enabled(&pool)
            .await
            .expect("foreign_keys pragma"),
        "foreign keys should be enabled"
    );

    for table in [
        "people",
        "projects",
        "schedule_config",
        "reports",
        "runs",
        "run_projects",
    ] {
        assert!(
            table_exists(&pool, table).await.expect("table_exists"),
            "missing table {table}"
        );
    }
}

#[tokio::test]
async fn data_dir_layout_created() {
    let temp = tempfile::tempdir().expect("tempdir");
    init_pool(temp.path())
        .await
        .expect("init pool creates layout");

    assert!(temp.path().join("repos").is_dir());
    assert!(temp.path().join("reports").is_dir());
    assert!(temp.path().join("reviewer.db").is_file());
}

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());

    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data_dir"], temp.path().display().to_string());
}

#[tokio::test]
async fn cors_allows_configured_origin() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let previous_cors = std::env::var("CORS_ALLOW_ORIGINS").ok();
    std::env::set_var("DATA_ROOT_DIR", temp.path());

    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
    std::env::set_var(
        "CORS_ALLOW_ORIGINS",
        "https://reviewer.example.com,http://localhost:5173",
    );

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("Origin", "https://reviewer.example.com")
                .header("Access-Control-Request-Method", "GET")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://reviewer.example.com")
    );

    match previous_cors {
        Some(value) => std::env::set_var("CORS_ALLOW_ORIGINS", value),
        None => std::env::remove_var("CORS_ALLOW_ORIGINS"),
    }
}

#[test]
fn reviewer_batch_skill_files_exist() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let skill_dir = repo_root.join("skills/reviewer-batch");
    assert!(
        skill_dir.join("WORKFLOW.md").is_file(),
        "missing skills/reviewer-batch/WORKFLOW.md"
    );
    assert!(
        skill_dir.join("output-contract.md").is_file(),
        "missing skills/reviewer-batch/output-contract.md"
    );
}

#[test]
fn scan_mrs_headless_skill_files_exist() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let skill_dir = repo_root.join("skills/scan-mrs-headless");
    assert!(
        skill_dir.join("WORKFLOW.md").is_file(),
        "missing skills/scan-mrs-headless/WORKFLOW.md"
    );
    assert!(
        skill_dir.join("output-contract.md").is_file(),
        "missing skills/scan-mrs-headless/output-contract.md"
    );
    assert!(
        skill_dir.join("observation-guidelines.md").is_file(),
        "missing skills/scan-mrs-headless/observation-guidelines.md"
    );
}
