use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::{init_pool, table_exists};
use reviewer_server::report_chat;
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

fn report_chat_executor_path(ok: bool) -> std::path::PathBuf {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let name = if ok {
        "report_chat_ok"
    } else {
        "report_chat_fail"
    };
    if cfg!(windows) {
        fixtures.join(format!("{name}.cmd"))
    } else {
        fixtures.join(format!("{name}.sh"))
    }
}

#[tokio::test]
async fn migration_014_creates_person_report_chat_tables() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    assert!(
        table_exists(&pool, "person_report_chats")
            .await
            .expect("table_exists"),
        "person_report_chats must exist"
    );
    assert!(
        table_exists(&pool, "person_report_chat_messages")
            .await
            .expect("table_exists"),
        "person_report_chat_messages must exist"
    );

    let has_version: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM schema_version WHERE version = 14",
    )
    .fetch_one(&pool)
    .await
    .expect("schema_version");
    assert_eq!(has_version, 1);
}

#[tokio::test]
async fn get_report_chat_returns_empty_history_for_known_person() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 = sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Alice') RETURNING id")
        .fetch_one(&pool)
        .await
        .expect("person");
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let response = report_chat::get_report_chat(&pool, &config, person_id)
        .await
        .expect("get chat");
    assert!(response.agent_session_id.is_none());
    assert!(response.chat_messages.is_empty());

    let err = report_chat::get_report_chat(&pool, &config, person_id + 99)
        .await
        .expect_err("unknown person");
    assert!(matches!(err, reviewer_server::Error::NotFound));

    std::env::remove_var("REVIEWER_EXECUTOR");
}

#[tokio::test]
async fn report_chat_first_turn_creates_session_and_messages() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    std::env::set_var("REVIEWER_EXECUTOR", report_chat_executor_path(true));
    std::env::set_var("REPORT_CHAT_SESSION_ID", "report-sess-new");
    std::env::set_var("REPORT_CHAT_REPLY", "hello from report chat");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 =
        sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Alice') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("person");
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let response = report_chat::agent_turn(
        &pool,
        &config,
        person_id,
        "please tweak the summary",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("agent turn");

    assert_eq!(response.reply, "hello from report chat");
    assert_eq!(response.agent_session_id.as_deref(), Some("report-sess-new"));

    let messages = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM person_report_chat_messages WHERE person_id = ? ORDER BY id ASC",
    )
    .bind(person_id)
    .fetch_all(&pool)
    .await
    .expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].0, "user");
    assert_eq!(messages[0].1, "please tweak the summary");
    assert_eq!(messages[1].0, "assistant");
    assert_eq!(messages[1].1, "hello from report chat");

    let session: Option<String> = sqlx::query_scalar(
        "SELECT agent_session_id FROM person_report_chats WHERE person_id = ?",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("session");
    assert_eq!(session.as_deref(), Some("report-sess-new"));

    std::env::remove_var("REVIEWER_EXECUTOR");
    std::env::remove_var("REPORT_CHAT_SESSION_ID");
    std::env::remove_var("REPORT_CHAT_REPLY");
}

#[tokio::test]
async fn report_chat_second_turn_resumes_and_appends() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    std::env::set_var("REVIEWER_EXECUTOR", report_chat_executor_path(true));
    std::env::set_var("REPORT_CHAT_SESSION_ID", "report-sess-1");
    std::env::set_var("REPORT_CHAT_REPLY", "first reply");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 =
        sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Bob') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("person");
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    report_chat::agent_turn(
        &pool,
        &config,
        person_id,
        "first",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("first turn");

    std::env::set_var("REPORT_CHAT_SESSION_ID", "report-sess-2");
    std::env::set_var("REPORT_CHAT_REPLY", "second reply");

    let response = report_chat::agent_turn(
        &pool,
        &config,
        person_id,
        "second",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("second turn");

    assert_eq!(response.reply, "second reply");
    assert_eq!(response.agent_session_id.as_deref(), Some("report-sess-2"));

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM person_report_chat_messages WHERE person_id = ?",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 4);

    std::env::remove_var("REVIEWER_EXECUTOR");
    std::env::remove_var("REPORT_CHAT_SESSION_ID");
    std::env::remove_var("REPORT_CHAT_REPLY");
}

#[tokio::test]
async fn report_chat_failure_does_not_persist_messages() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    std::env::set_var("REVIEWER_EXECUTOR", report_chat_executor_path(false));

    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 =
        sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Carol') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("person");
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let err = report_chat::agent_turn(
        &pool,
        &config,
        person_id,
        "please fail",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect_err("must fail");
    assert!(matches!(err, reviewer_server::Error::AgentFailed(_)));

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM person_report_chat_messages WHERE person_id = ?",
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 0);

    let chat_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM person_report_chats WHERE person_id = ?")
            .bind(person_id)
            .fetch_one(&pool)
            .await
            .expect("chat rows");
    assert_eq!(chat_rows, 0);

    std::env::remove_var("REVIEWER_EXECUTOR");
}

#[tokio::test]
async fn report_chat_reingests_summary_preserving_run_id() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    std::env::set_var("REVIEWER_EXECUTOR", report_chat_executor_path(true));
    std::env::set_var("REPORT_CHAT_REPLY", "rewrote one_line");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 =
        sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Dana') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("person");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo, default_branch) VALUES ('alpha', '.', 0, 'main')",
    )
    .execute(&pool)
    .await
    .expect("project");
    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");
    sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'success', 1)",
    )
    .execute(&pool)
    .await
    .expect("run");
    let run_id: i64 = sqlx::query_scalar("SELECT id FROM runs ORDER BY id DESC LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("run id");

    let summary_dir = temp
        .path()
        .join("reports")
        .join("alpha")
        .join("Dana")
        .join("2026-07-08");
    std::fs::create_dir_all(&summary_dir).expect("summary dir");
    let summary_path = summary_dir.join("summary.md");
    let original = r#"---
person: Dana
project: alpha
date: "2026-07-08"
one_line: original summary
mr_count: 1
commit_count: 2
---

## 本週重點
- shipped feature

## 成長面向
- mentorship

## 待確認
- need clarification?

## 已釐清
"#;
    std::fs::write(&summary_path, original).expect("write summary");

    sqlx::query(
        "INSERT INTO reports (project_id, person_id, run_id, report_date, report_md_path, summary_md_path, one_line, mr_count, commit_count)
         VALUES (?, ?, ?, '2026-07-08', ?, ?, 'original summary', 1, 2)",
    )
    .bind(project_id)
    .bind(person_id)
    .bind(run_id)
    .bind(summary_dir.join("report.md").display().to_string())
    .bind(summary_path.display().to_string())
    .execute(&pool)
    .await
    .expect("report");

    let updated = r#"---
person: Dana
project: alpha
date: "2026-07-08"
one_line: revised by agent chat
mr_count: 3
commit_count: 4
---

## 本週重點
- shipped feature

## 成長面向
- mentorship

## 待確認
- follow up on cache design

## 已釐清
"#;
    std::env::set_var(
        "REPORT_CHAT_SUMMARY_PATH",
        summary_path.display().to_string(),
    );
    std::env::set_var("REPORT_CHAT_SUMMARY_BODY", updated);

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let response = report_chat::agent_turn(
        &pool,
        &config,
        person_id,
        "update one_line and pending",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("agent turn");
    assert!(response.ingest_warnings.is_empty(), "{:?}", response.ingest_warnings);

    let row: (String, i64, i64, i64) = sqlx::query_as(
        "SELECT one_line, mr_count, commit_count, run_id FROM reports
         WHERE project_id = ? AND person_id = ? AND report_date = '2026-07-08'",
    )
    .bind(project_id)
    .bind(person_id)
    .fetch_one(&pool)
    .await
    .expect("report row");
    assert_eq!(row.0, "revised by agent chat");
    assert_eq!(row.1, 3);
    assert_eq!(row.2, 4);
    assert_eq!(row.3, run_id, "run_id must be preserved");

    let pending: Vec<String> = sqlx::query_scalar(
        "SELECT question FROM pending_items WHERE person_id = ? AND status = 'open'",
    )
    .bind(person_id)
    .fetch_all(&pool)
    .await
    .expect("pending");
    assert!(
        pending.iter().any(|q| q.contains("follow up on cache design")),
        "pending={pending:?}"
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
    std::env::remove_var("REPORT_CHAT_SUMMARY_PATH");
    std::env::remove_var("REPORT_CHAT_SUMMARY_BODY");
    std::env::remove_var("REPORT_CHAT_REPLY");
}

#[tokio::test]
async fn report_chat_http_empty_message_is_400() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 =
        sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Eve') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("person");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/people/{person_id}/report-chat/agent-turn"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message":"   "}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn report_chat_http_get_404_for_unknown_person() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/people/999/report-chat")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let _ = response.into_body().collect().await;
}

#[tokio::test]
async fn report_chat_http_get_returns_json() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    let pool = init_pool(temp.path()).await.expect("init pool");
    let person_id: i64 =
        sqlx::query_scalar("INSERT INTO people (display_name) VALUES ('Frank') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("person");
    let app = build_app().await.expect("build app");

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/people/{person_id}/report-chat"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let value: Value = serde_json::from_slice(&body).expect("json");
    assert!(value["chat_messages"].as_array().unwrap().is_empty());
    assert!(value["agent_session_id"].is_null());
}
