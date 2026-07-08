use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use reviewer_server::person_trends::{self, PERSON_REPORT_DIR};
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

async fn seed_person(pool: &sqlx::SqlitePool, display_name: &str) -> i64 {
    let result = sqlx::query("INSERT INTO people (display_name) VALUES (?)")
        .bind(display_name)
        .execute(pool)
        .await
        .expect("insert person");
    result.last_insert_rowid()
}

#[tokio::test]
async fn person_directory_is_separate_from_project_directories() {
    assert!(person_trends::is_person_level_report_name(PERSON_REPORT_DIR));
    assert!(!person_trends::is_person_level_report_name("game-backend"));
    assert!(!person_trends::is_person_level_report_name("Alice Chen"));

    let temp = tempfile::tempdir().expect("tempdir");
    let root = person_trends::person_report_root(temp.path());
    assert!(root.ends_with(format!("reports/{PERSON_REPORT_DIR}")));
}

#[tokio::test]
async fn trends_api_returns_person_level_index_content() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice Chen").await;

    let index_path = person_trends::person_trends_dir(temp.path(), "Alice Chen").join("index.md");
    std::fs::create_dir_all(index_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&index_path, "# Cross-project observation\n\nAlice is growing.").expect("write");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/trends"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["display_name"], "Alice Chen");
    assert!(json["long_term_observation"]
        .as_str()
        .unwrap_or("")
        .contains("Cross-project observation"));
}

#[tokio::test]
async fn missing_person_level_files_return_empty_sections() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Bob").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/trends"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["long_term_observation"], "");
    assert_eq!(json["growth_timeline"].as_array().expect("array").len(), 0);
    assert_eq!(json["historical_pending"].as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn legacy_markdown_displays_without_frontmatter() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Bob").await;

    let index_path = person_trends::person_trends_dir(temp.path(), "Bob").join("index.md");
    std::fs::create_dir_all(index_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &index_path,
        "# Legacy notes\n\nFree-form markdown without YAML frontmatter.",
    )
    .expect("write");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/trends"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert!(json["long_term_observation"]
        .as_str()
        .unwrap_or("")
        .contains("Free-form markdown"));
}

#[tokio::test]
async fn trends_api_returns_404_for_unknown_person() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/people/999/trends")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn latest_reports_excludes_long_term_observation() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;

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
---

## 本週重點
- Led database tuning
"#,
    )
    .expect("write summary");

    sqlx::query(
        "INSERT INTO reports (
            project_id, person_id, report_date, report_md_path, summary_md_path,
            one_line, is_read
         ) VALUES (1, ?, '2026-07-05', ?, ?, 'Stable week', 0)",
    )
    .bind(person_id)
    .bind(
        temp.path()
            .join("reports/game-backend/Alice/2026-07-05/report.md")
            .display()
            .to_string(),
    )
    .bind(summary_path.display().to_string())
    .execute(&pool)
    .await
    .expect("insert report");

    let index_path = person_trends::person_trends_dir(temp.path(), "Alice").join("index.md");
    std::fs::create_dir_all(index_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&index_path, "Long-term cross-project observation").expect("write");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/reports/latest"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let text = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(!text.contains("Long-term cross-project observation"));
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["projects"].as_array().expect("projects").len(), 1);
}
