use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use serde_json::{json, Value};
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

#[tokio::test]
async fn migration_seeds_single_row_with_empty_list() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM review_settings")
        .fetch_one(&pool)
        .await
        .expect("count rows");
    assert_eq!(rows, 1, "migration must seed exactly one settings row");

    let raw: String = sqlx::query_scalar("SELECT ignore_globs FROM review_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("read ignore_globs");
    let parsed: Vec<String> = serde_json::from_str(&raw).expect("stored value is a JSON array");
    assert!(parsed.is_empty(), "fresh install must start with no ignore rules");
}

async fn put_ignore_globs(body: Value) -> (StatusCode, Value) {
    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/review-settings")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get_ignore_globs() -> Value {
    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/review-settings")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn put_replaces_list_and_get_returns_it() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let (status, body) = put_ignore_globs(json!({ "ignore_globs": ["*.lock"] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ignore_globs"], json!(["*.lock"]));

    // Full replacement, not a merge.
    let (status, body) = put_ignore_globs(json!({ "ignore_globs": ["vendor/**"] })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ignore_globs"], json!(["vendor/**"]));

    assert_eq!(get_ignore_globs().await["ignore_globs"], json!(["vendor/**"]));
}

#[tokio::test]
async fn put_normalizes_silently() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let (status, body) =
        put_ignore_globs(json!({ "ignore_globs": ["  *.lock  ", "*.lock", "", "   ", "b.lock"] }))
            .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ignore_globs"], json!(["*.lock", "b.lock"]));
    assert_eq!(get_ignore_globs().await["ignore_globs"], json!(["*.lock", "b.lock"]));
}

#[tokio::test]
async fn put_rejects_invalid_entries_and_keeps_stored_value() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let (status, _) = put_ignore_globs(json!({ "ignore_globs": ["*.lock"] })).await;
    assert_eq!(status, StatusCode::OK);

    let too_long = "a".repeat(201);
    let too_many: Vec<String> = (0..101).map(|i| format!("f{i}.lock")).collect();
    let rejected = [
        json!({ "ignore_globs": [":(exclude)*.lock"] }),
        json!({ "ignore_globs": [":(top)"] }),
        json!({ "ignore_globs": [too_long] }),
        json!({ "ignore_globs": too_many }),
    ];

    for body in rejected {
        let (status, _) = put_ignore_globs(body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "must reject {body}");
        assert_eq!(
            get_ignore_globs().await["ignore_globs"],
            json!(["*.lock"]),
            "rejected write must leave the stored list untouched"
        );
    }
}
