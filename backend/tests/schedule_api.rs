use std::sync::Mutex;

use axum::body::Body;
use chrono::Datelike;
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
async fn schedule_api_returns_mr_poll_interval() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let _pool = init_pool(temp.path()).await.expect("init pool");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schedule")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["mr_poll_interval_min"], 60);
    assert_eq!(json["mr_poll_label"], "每 1 小時");
}

#[tokio::test]
async fn schedule_api_updates_mr_poll_interval() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let _pool = init_pool(temp.path()).await.expect("init pool");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/schedule")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"mr_poll_interval_min":30}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["mr_poll_interval_min"], 30);
    assert_eq!(json["mr_poll_label"], "每 30 分鐘");
}

#[tokio::test]
async fn schedule_api_updates_weekday_and_run_time() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/schedule")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"weekday":2,"run_time":"10:30"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["weekday"], 2);
    assert_eq!(json["run_time"], "10:30");
    assert_eq!(json["weekly_label"], "每週三 10:30");

    let (weekday, run_time): (i64, String) = sqlx::query_as(
        "SELECT weekday, run_time FROM schedule_config WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("row");
    assert_eq!(weekday, 2);
    assert_eq!(run_time, "10:30");
}

#[tokio::test]
async fn schedule_api_rejects_non_weekly_cadence() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/schedule")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"cadence":"daily"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let cadence: String =
        sqlx::query_scalar("SELECT cadence FROM schedule_config WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("cadence");
    assert_eq!(cadence, "weekly");
}

#[tokio::test]
async fn schedule_api_rejects_zero_timeout() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let _pool = init_pool(temp.path()).await.expect("init pool");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/schedule")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"per_project_timeout_sec":0}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn schedule_api_reports_missed_weekly_run_when_uncovered() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let yesterday_weekday = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
        .date_naive()
        .pred_opt()
        .expect("yesterday")
        .weekday()
        .num_days_from_monday() as i64;
    sqlx::query(
        "UPDATE schedule_config
         SET enabled = 1, weekday = ?, run_time = '00:00', tz_offset_min = 480
         WHERE id = 1",
    )
    .bind(yesterday_weekday)
    .execute(&pool)
    .await
    .expect("update schedule");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schedule")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert!(
        json["missed_weekly_run"].is_object(),
        "expected missed_weekly_run object, got {json}"
    );
    assert!(json["missed_weekly_run"]["due_at"].as_str().is_some());
    assert!(json["missed_weekly_run"]["label"].as_str().is_some());
}

#[tokio::test]
async fn schedule_api_suppresses_missed_when_manual_all_covers() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    let yesterday_weekday = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
        .date_naive()
        .pred_opt()
        .expect("yesterday")
        .weekday()
        .num_days_from_monday() as i64;
    sqlx::query(
        "UPDATE schedule_config
         SET enabled = 1, weekday = ?, run_time = '00:00', tz_offset_min = 480
         WHERE id = 1",
    )
    .bind(yesterday_weekday)
    .execute(&pool)
    .await
    .expect("update schedule");

    let started_at = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, duration_sec)
         VALUES ('manual_all', 'success', ?, ?, 60)",
    )
    .bind(&started_at)
    .bind(&started_at)
    .execute(&pool)
    .await
    .expect("insert covering run");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schedule")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert!(json["missed_weekly_run"].is_null());
}

#[tokio::test]
async fn schedule_api_missed_null_when_disabled() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query("UPDATE schedule_config SET enabled = 0 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("disable");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schedule")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert!(json["missed_weekly_run"].is_null());
}

#[tokio::test]
async fn schedule_catch_up_creates_manual_all_run() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('demo', ?, 0)",
    )
    .bind(temp.path().join("repos/demo").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/schedule/catch-up")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let run_id = json["run_id"].as_i64().expect("run_id");
    assert!(run_id > 0);

    let trigger: String = sqlx::query_scalar("SELECT trigger FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("trigger");
    assert_eq!(trigger, "manual_all");
}

#[tokio::test]
async fn schedule_catch_up_conflict_returns_409() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('demo', ?, 0)",
    )
    .bind(temp.path().join("repos/demo").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert active run");
    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (1, 1, 'running')",
    )
    .execute(&pool)
    .await
    .expect("insert run project");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/schedule/catch-up")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}
