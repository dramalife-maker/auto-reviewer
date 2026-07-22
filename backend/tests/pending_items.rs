use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use reviewer_server::summary::ingest_project_summaries;
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

async fn seed_project(pool: &sqlx::SqlitePool, temp: &tempfile::TempDir, name: &str) -> i64 {
    let result = sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES (?, ?, 0)",
    )
    .bind(name)
    .bind(temp.path().join("repos").join(name).display().to_string())
    .execute(pool)
    .await
    .expect("insert project");
    result.last_insert_rowid()
}

async fn seed_pending_item(
    pool: &sqlx::SqlitePool,
    person_id: i64,
    project_id: i64,
    question: &str,
    status: &str,
    raised_date: &str,
) -> i64 {
    let result = sqlx::query(
        "INSERT INTO pending_items (person_id, project_id, question, status, raised_date)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(person_id)
    .bind(project_id)
    .bind(question)
    .bind(status)
    .bind(raised_date)
    .execute(pool)
    .await
    .expect("insert pending item");
    result.last_insert_rowid()
}

async fn seed_run(pool: &sqlx::SqlitePool) -> i64 {
    let result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(pool)
    .await
    .expect("insert run");
    result.last_insert_rowid()
}

/// Writes a `summary.md` at `{report_root}/{person}/{date}/summary.md`, matching the
/// on-disk layout `find_summary_files` expects (see `skills/reviewer-batch/output-contract.md`).
fn write_summary(
    temp: &tempfile::TempDir,
    project: &str,
    person: &str,
    date: &str,
    pending: Option<&str>,
    cleared: Option<&str>,
) {
    let summary_path = temp
        .path()
        .join(format!("reports/{project}/{person}/{date}/summary.md"));
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    let pending_body = pending
        .map(|q| format!("- {q}\n"))
        .unwrap_or_default();
    let cleared_body = cleared
        .map(|q| format!("- {q}\n"))
        .unwrap_or_default();
    std::fs::write(
        &summary_path,
        format!(
            r#"---
person: {person}
project: {project}
date: {date}
one_line: Stable week
commit_count: 1
---

## 本週重點
- Shipped feature X

## 成長面向

## 待確認
{pending_body}
## 已釐清
{cleared_body}"#
        ),
    )
    .expect("write summary");
}

#[tokio::test]
async fn list_pending_items_default_returns_only_open() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;
    seed_pending_item(&pool, person_id, project_id, "Old resolved Q", "resolved", "2026-06").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/pending-items"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let items = json.as_array().expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["question"], "Why choose A?");
    assert_eq!(items[0]["status"], "open");
    assert_eq!(items[0]["project_name"], "game-backend");
}

#[tokio::test]
async fn list_pending_items_filters_by_resolved_status() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;
    seed_pending_item(&pool, person_id, project_id, "Old resolved Q", "resolved", "2026-06").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/pending-items?status=resolved"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let items = json.as_array().expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["question"], "Old resolved Q");
    assert_eq!(items[0]["status"], "resolved");
}

#[tokio::test]
async fn list_pending_items_unknown_person_returns_404() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/people/999/pending-items")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn resolve_pending_item_success() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["status"], "resolved");
    assert!(json["resolved_date"].as_str().is_some());
    assert!(json["resolution_note"].is_null());
}

#[tokio::test]
async fn resolve_pending_item_with_note() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"status":"resolved","resolution_note":"Chose option B in 1on1"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["resolution_note"], "Chose option B in 1on1");
}

#[tokio::test]
async fn resolve_pending_item_already_resolved_returns_409() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id = seed_pending_item(
        &pool,
        person_id,
        project_id,
        "Already done",
        "resolved",
        "2026-06",
    )
    .await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let status: String = sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "resolved");
}

#[tokio::test]
async fn resolve_pending_item_invalid_status_returns_400() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"open"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn resolve_pending_item_unknown_id_returns_404() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/pending-items/999")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn resolve_pending_item_syncs_notes_file_matching_line() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id = seed_pending_item(
        &pool,
        person_id,
        project_id,
        "Why choose A?",
        "open",
        "2026-07",
    )
    .await;

    let notes_path = temp
        .path()
        .join("reports/_people/Alice Chen/_notes.md");
    std::fs::create_dir_all(notes_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&notes_path, "- [2026-07] Why choose A?\n").expect("write notes");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let content = std::fs::read_to_string(&notes_path).expect("read notes");
    assert!(content.contains("\u{2192}"));
    assert!(content.contains("\u{2713} Why choose A?"));
}

#[tokio::test]
async fn resolve_pending_item_creates_missing_notes_file() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Bob").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let notes_path = temp.path().join("reports/_people/Bob/_notes.md");
    assert!(!notes_path.exists());

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let content = std::fs::read_to_string(&notes_path).expect("read notes");
    assert!(content.contains("Why choose A?"));
}

#[cfg(unix)]
#[tokio::test]
async fn resolve_pending_item_returns_502_when_notes_write_fails() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Carol").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let people_dir = temp.path().join("reports/_people");
    std::fs::create_dir_all(&people_dir).expect("mkdir");
    let mut perms = std::fs::metadata(&people_dir).expect("meta").permissions();
    perms.set_mode(0o400);
    std::fs::set_permissions(&people_dir, perms).expect("chmod");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    // restore perms so tempdir cleanup succeeds
    let mut restore = std::fs::metadata(&people_dir).expect("meta").permissions();
    restore.set_mode(0o700);
    std::fs::set_permissions(&people_dir, restore).expect("chmod restore");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let status: String = sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "resolved");
}

#[tokio::test]
async fn list_pending_items_status_all_returns_open_and_resolved() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    seed_pending_item(&pool, person_id, project_id, "Open Q", "open", "2026-07").await;
    seed_pending_item(&pool, person_id, project_id, "Resolved Q", "resolved", "2026-06").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/pending-items?status=all"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json.as_array().expect("array").len(), 2);
}

#[tokio::test]
async fn list_pending_items_invalid_status_returns_400() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/pending-items?status=foo"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn resolve_pending_item_second_attempt_returns_409() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let app = build_app().await.expect("build app");
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn resolve_pending_item_then_trends_shows_resolved() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let item_id =
        seed_pending_item(&pool, person_id, project_id, "Why choose A?", "open", "2026-07").await;

    let notes_path = temp.path().join("reports/_people/Alice/_notes.md");
    std::fs::create_dir_all(notes_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&notes_path, "- [2026-07] Why choose A?\n").expect("write notes");

    let app = build_app().await.expect("build app");
    let resolve = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/pending-items/{item_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"resolved"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resolve.status(), StatusCode::OK);

    let trends = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/trends"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(trends.status(), StatusCode::OK);
    let body = trends.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let entries = json["historical_pending"].as_array().expect("array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["status"], "resolved");
    assert_eq!(entries[0]["question"], "Why choose A?");
}

#[tokio::test]
async fn backfill_pending_items_seeds_from_existing_summary() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-05/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        "---\nperson: Alice\nproject: game-backend\ndate: 2026-07-05\n---\n## 待確認\n- Why choose A?\n",
    )
    .expect("write summary");

    sqlx::query(
        "INSERT INTO reports (
            project_id, person_id, report_date, report_md_path, summary_md_path
         ) VALUES (?, ?, '2026-07-05', ?, ?)",
    )
    .bind(project_id)
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

    let inserted = reviewer_server::summary::backfill_pending_items(&pool)
        .await
        .expect("backfill");
    assert_eq!(inserted, 1);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE person_id = ? AND status = 'open'",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn reingesting_same_summary_does_not_duplicate_pending_item() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let run_id = seed_run(&pool).await;

    write_summary(
        &temp,
        "game-backend",
        "Alice Chen",
        "2026-07-05",
        Some("Why choose A?"),
        None,
    );

    // Ingest the same summary.md twice (e.g. a manual re-run of the same report),
    // matching issue #3 reproduction "Method A".
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("first ingest");
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("second ingest");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(
        count, 1,
        "re-ingesting the same summary.md must not duplicate the open pending item"
    );
}

#[tokio::test]
async fn carrying_open_question_into_next_week_does_not_duplicate_pending_item() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;

    // Week 1: the question is raised for the first time.
    let run_id_week1 = seed_run(&pool).await;
    write_summary(
        &temp,
        "game-backend",
        "Alice Chen",
        "2026-07-05",
        Some("Why choose A?"),
        None,
    );
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id_week1)
        .await
        .expect("week 1 ingest");

    let report_id_week1: i64 = sqlx::query_scalar(
        "SELECT report_id FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("week 1 report_id");

    // Week 2: normal weekly schedule (not a manual re-run). The workflow's
    // "待確認延續規則" requires the agent to write the same question verbatim
    // into a *new* summary.md dated one week later, because manifest.open_pending
    // still lists it as open — matching issue #3 reproduction "Method B".
    let run_id_week2 = seed_run(&pool).await;
    write_summary(
        &temp,
        "game-backend",
        "Alice Chen",
        "2026-07-12",
        Some("Why choose A?"),
        None,
    );
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id_week2)
        .await
        .expect("week 2 ingest");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(
        count, 1,
        "carrying the same open question into next week's summary must not duplicate the pending item"
    );

    // The existing open row should be re-pointed at the latest report, not left
    // dangling on the week-1 report_id.
    let report_id_week2: i64 = sqlx::query_scalar(
        "SELECT report_id FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("week 2 report_id");
    assert_ne!(report_id_week1, report_id_week2);
}

#[tokio::test]
async fn resolved_question_may_be_raised_again_as_new_open_row() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;

    // Week 1: raise, then clear via ## 已釐清 (same ingest pass can do both when
    // the agent moves the bullet; here we raise first then clear in a follow-up).
    let run_id_week1 = seed_run(&pool).await;
    write_summary(
        &temp,
        "game-backend",
        "Alice Chen",
        "2026-07-05",
        Some("Why choose A?"),
        None,
    );
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id_week1)
        .await
        .expect("week 1 raise");

    write_summary(
        &temp,
        "game-backend",
        "Alice Chen",
        "2026-07-05",
        None,
        Some("Why choose A?"),
    );
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id_week1)
        .await
        .expect("week 1 clear");

    let resolved_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ? AND status = 'resolved'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("resolved count");
    assert_eq!(resolved_count, 1);

    // Week 2: the same question text is raised again — must create a new open row
    // (partial unique index only covers status='open').
    let run_id_week2 = seed_run(&pool).await;
    write_summary(
        &temp,
        "game-backend",
        "Alice Chen",
        "2026-07-12",
        Some("Why choose A?"),
        None,
    );
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id_week2)
        .await
        .expect("week 2 re-raise");

    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("open count");
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ?",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("total count");
    assert_eq!(open_count, 1, "resolved history must not block a new open row");
    assert_eq!(total_count, 2, "resolved row must remain alongside the new open row");
}

/// Replay guard: ingest scans every historical `summary.md` under the project's
/// report root, so an already-processed summary is re-read on every later run.
/// Once the question is `resolved` it falls outside `idx_pending_open_unique`
/// (partial index, `WHERE status='open'`), so the insert is no longer ignored.
#[tokio::test]
async fn reingesting_processed_summary_after_resolve_does_not_duplicate() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let run_id = seed_run(&pool).await;

    // Raise, then clear, both from the report dated 2026-07-05.
    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("raise");

    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", None, Some("Why choose A?"));
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("clear");

    // A later run re-reads the same 2026-07-05 summary that still lists the question.
    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("replay");

    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_items WHERE question = ?")
            .bind("Why choose A?")
            .fetch_one(&pool)
            .await
            .expect("total count");
    assert_eq!(
        total_count, 1,
        "re-reading an already-processed summary must not create a second row"
    );
}

/// A summary older than the row's originating report is history, not news.
#[tokio::test]
async fn reingesting_older_summary_does_not_reopen_resolved_question() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let run_id = seed_run(&pool).await;

    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-12", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("raise");

    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-12", None, Some("Why choose A?"));
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("clear");

    // An older summary still lists the question under 待確認.
    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("replay older");

    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_items WHERE question = ?")
            .bind("Why choose A?")
            .fetch_one(&pool)
            .await
            .expect("total count");
    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("open count");
    assert_eq!(total_count, 1, "an older summary must not create a second row");
    assert_eq!(open_count, 0, "an older summary must not reopen a resolved question");
}

/// Carrying an open question forward rewrites its `report_id` to the newer report.
/// Comparing the incoming report date for equality alone would therefore miss the
/// replay of the *original* summary — hence the `>=` comparison.
#[tokio::test]
async fn replaying_original_summary_after_carry_forward_and_resolve_does_not_duplicate() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let run_id = seed_run(&pool).await;

    // Week 1: raised from 2026-07-05.
    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("week 1 raise");

    // Week 2 carries the still-open question forward, moving report_id to 2026-07-12.
    // Remove the week 1 directory so the carry-forward target is unambiguous.
    std::fs::remove_dir_all(temp.path().join("reports/game-backend/Alice Chen/2026-07-05"))
        .expect("remove week 1 report dir");
    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-12", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("week 2 carry forward");

    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-12", None, Some("Why choose A?"));
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("week 2 clear");

    // The original 2026-07-05 summary reappears on a later scan.
    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("replay original");

    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_items WHERE question = ?")
            .bind("Why choose A?")
            .fetch_one(&pool)
            .await
            .expect("total count");
    assert_eq!(
        total_count, 1,
        "replaying the original summary after carry-forward must not create a second row"
    );
}

/// The replay guard needs the originating report's date. When the report
/// reference is `NULL` the date is unknowable, and the guard must fail open so a
/// genuinely new question is never silently dropped.
#[tokio::test]
async fn pending_row_without_report_reference_does_not_block_insertion() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id = seed_person(&pool, "Alice Chen").await;
    let project_id = seed_project(&pool, &temp, "game-backend").await;
    let run_id = seed_run(&pool).await;

    // seed_pending_item leaves report_id NULL.
    seed_pending_item(&pool, person_id, project_id, "Why choose A?", "resolved", "2026-07").await;

    write_summary(&temp, "game-backend", "Alice Chen", "2026-07-05", Some("Why choose A?"), None);
    ingest_project_summaries(&pool, temp.path(), "game-backend", project_id, run_id)
        .await
        .expect("ingest");

    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = ? AND status = 'open'",
    )
    .bind("Why choose A?")
    .fetch_one(&pool)
    .await
    .expect("open count");
    assert_eq!(
        open_count, 1,
        "a row with no originating report must not block a new open row"
    );
}
