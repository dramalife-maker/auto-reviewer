use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use reviewer_server::reports;
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

async fn seed_person_with_report(temp: &tempfile::TempDir, pool: &sqlx::SqlitePool) -> (i64, i64) {
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(pool)
    .await
    .expect("insert project");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

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

## 本週重點
- Led database tuning

## 成長面向
- Better PR sizing

## 待確認
- Architecture choice on MR 234?
"#,
    )
    .expect("write summary");

    let report_result = sqlx::query(
        "INSERT INTO reports (
            project_id, person_id, report_date, report_md_path, summary_md_path,
            one_line, mr_count, commit_count, is_read
         ) VALUES (1, ?, '2026-07-05', ?, ?, 'Stable week', 6, 42, 0)",
    )
    .bind(person_id)
    .bind(
        temp.path()
            .join("reports/game-backend/Alice/2026-07-05/report.md")
            .display()
            .to_string(),
    )
    .bind(summary_path.display().to_string())
    .execute(pool)
    .await
    .expect("insert report");

    (person_id, report_result.last_insert_rowid())
}

#[tokio::test]
async fn people_list_includes_unread() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person_with_report(&temp, &pool).await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/people")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let alice = &json[0];
    assert_eq!(alice["display_name"], "Alice");
    assert!(alice["unread_count"].as_i64().unwrap_or(0) > 0);
    assert_eq!(alice["identity_count"].as_i64().unwrap_or(-1), 0);
}

#[tokio::test]
async fn latest_reports_returns_sections() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let (person_id, _) = seed_person_with_report(&temp, &pool).await;

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
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["report_date"], "2026-07-05");
    assert_eq!(json["projects"][0]["project_name"], "game-backend");
    assert_eq!(json["projects"][0]["one_line"], "Stable week");
    assert!(!json["projects"][0]["highlights"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn mark_report_read() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let (person_id, report_id) = seed_person_with_report(&temp, &pool).await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/reports/{report_id}/read"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        reports::unread_count_for_person(&pool, person_id)
            .await
            .expect("unread"),
        0
    );
}

#[tokio::test]
async fn get_run_by_id_returns_terminal_status() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total, project_skipped, finished_at)
         VALUES ('manual_all', 'success', 2, 0, datetime('now'))",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/runs/{run_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["status"], "success");
    assert_eq!(json["trigger"], "manual_all");
}
