use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

#[tokio::test]
async fn dashboard_returns_stats_and_schedule() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(&pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-05/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&summary_path, "## 本週重點\n- item\n").expect("write summary");

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

    sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, duration_sec)
         VALUES ('manual_all', 'success', '2026-07-05 09:00:00', '2026-07-05 09:04:12', 252)",
    )
    .execute(&pool)
    .await
    .expect("insert run");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");

    assert_eq!(json["stats"]["project_count"], 1);
    assert_eq!(json["stats"]["person_count"], 1);
    assert_eq!(json["stats"]["unread_count"], 1);
    assert_eq!(json["last_run"]["duration_sec"], 252);
    assert_eq!(json["recent_reports"][0]["person_name"], "Alice");
    assert_eq!(json["schedule"]["label"], "每週一 09:00");
    assert!(json["schedule"]["enabled"].as_bool().unwrap_or(false));
}
