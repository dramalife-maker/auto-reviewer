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
