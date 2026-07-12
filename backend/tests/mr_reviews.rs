use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::config::ReviewerAgent;
use reviewer_server::db::init_pool;
use reviewer_server::executor::parse_agent_session_id;
use reviewer_server::mr_reviews::{self, ingest_mr_draft};
use serde_json::Value;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

#[test]
fn parse_agent_session_id_reads_claude_result_event() {
    let stdout = r#"{"type":"result","session_id":"sess-claude-42"}"#;
    assert_eq!(
        parse_agent_session_id(stdout, ReviewerAgent::Claude).as_deref(),
        Some("sess-claude-42")
    );
}

#[test]
fn parse_agent_session_id_reads_cursor_system_init() {
    let stdout =
        r#"{"type":"system","subtype":"init","session_id":"sess-cursor-7"}"#;
    assert_eq!(
        parse_agent_session_id(stdout, ReviewerAgent::Cursor).as_deref(),
        Some("sess-cursor-7")
    );
}

#[tokio::test]
async fn ingest_mr_draft_upserts_same_round() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let draft_dir = temp.path().join("drafts");
    std::fs::create_dir_all(&draft_dir).expect("draft dir");
    let draft_path = draft_dir.join("mr-42-round-1.md");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: feat cache\nreview_round: 1\nauthor_identity: alice@co.com\n---\nFirst body\n",
    )
    .expect("write draft");

    ingest_mr_draft(
        &pool,
        project_id,
        &draft_path,
        Some("sess-1"),
        ReviewerAgent::Cursor,
        false,
    )
    .await
    .expect("ingest");

    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: feat cache\nreview_round: 1\nauthor_identity: alice@co.com\n---\nSecond body\n",
    )
    .expect("rewrite draft");

    ingest_mr_draft(
        &pool,
        project_id,
        &draft_path,
        Some("sess-2"),
        ReviewerAgent::Claude,
        false,
    )
    .await
    .expect("re-ingest");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mr_reviews WHERE project_id = ? AND mr_iid = 42 AND review_round = 1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 1);

    let session_id: String =
        sqlx::query_scalar("SELECT agent_session_id FROM mr_reviews WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .expect("session");
    assert_eq!(session_id, "sess-2");

    let reviewer_agent: String =
        sqlx::query_scalar("SELECT reviewer_agent FROM mr_reviews WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .expect("agent");
    assert_eq!(reviewer_agent, "claude");
}

#[tokio::test]
async fn update_draft_preserves_frontmatter_on_disk() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let draft_path = temp.path().join("drafts/mr-42.md");
    std::fs::create_dir_all(draft_path.parent().expect("parent")).expect("dir");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: feat cache\nreview_round: 1\nauthor_identity: alice@co.com\n---\nOld body\n",
    )
    .expect("write draft");

    ingest_mr_draft(
        &pool,
        project_id,
        &draft_path,
        Some("sess-1"),
        ReviewerAgent::Cursor,
        false,
    )
    .await
    .expect("ingest");

    let review_id: i64 = sqlx::query_scalar("SELECT id FROM mr_reviews WHERE mr_iid = 42")
        .fetch_one(&pool)
        .await
        .expect("id");

    mr_reviews::update_draft(&pool, review_id, "# Edited\n\nNo yaml here")
        .await
        .expect("update");

    let on_disk = std::fs::read_to_string(&draft_path).expect("read");
    assert!(on_disk.contains("mr_iid: 42"));
    assert!(on_disk.contains("author_identity: alice@co.com"));
    assert!(on_disk.contains("# Edited"));
    assert!(!on_disk.contains("Old body"));

    let items = mr_reviews::list_mr_reviews(&pool, Some("draft"))
        .await
        .expect("list");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].draft_body.trim(), "# Edited\n\nNo yaml here");
    assert!(!items[0].draft_body.contains("mr_iid:"));
}

#[tokio::test]
async fn load_prior_published_reviews_returns_oldest_first() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let draft_dir = temp.path().join("drafts");
    std::fs::create_dir_all(&draft_dir).expect("draft dir");

    for (round, body, status) in [
        (1_i64, "round-1 published body", "published"),
        (2, "round-2 draft only", "draft"),
        (1, "should not appear — wrong mr", "published"),
    ] {
        let mr_iid = if body.contains("wrong") { 99 } else { 42 };
        let path = draft_dir.join(format!("mr-{mr_iid}-r{round}-{status}.md"));
        std::fs::write(
            &path,
            format!(
                "---\nmr_iid: {mr_iid}\nmr_title: t\nreview_round: {round}\nauthor_identity: a@b.com\n---\n{body}\n"
            ),
        )
        .expect("write");
        ingest_mr_draft(
            &pool,
            project_id,
            &path,
            None,
            ReviewerAgent::Cursor,
            false,
        )
        .await
        .expect("ingest");
        if status == "published" {
            sqlx::query(
                "UPDATE mr_reviews SET status = 'published', published_body = ?, published_at = datetime('now') WHERE draft_md_path = ?",
            )
            .bind(body)
            .bind(path.display().to_string())
            .execute(&pool)
            .await
            .expect("publish row");
        }
    }

    // Second published round for same MR.
    let path_r2 = draft_dir.join("mr-42-r2-published.md");
    std::fs::write(
        &path_r2,
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 2\nauthor_identity: a@b.com\n---\nround-2 published body\n",
    )
    .expect("write r2");
    // Clear draft-only round 2 row conflict by updating that row instead of inserting — delete draft-only first.
    sqlx::query("DELETE FROM mr_reviews WHERE project_id = ? AND mr_iid = 42 AND review_round = 2")
        .bind(project_id)
        .execute(&pool)
        .await
        .expect("delete draft r2");
    ingest_mr_draft(
        &pool,
        project_id,
        &path_r2,
        None,
        ReviewerAgent::Cursor,
        false,
    )
    .await
    .expect("ingest r2");
    sqlx::query(
        "UPDATE mr_reviews SET status = 'published', published_body = ?, published_at = datetime('now') WHERE draft_md_path = ?",
    )
    .bind("round-2 published body")
    .bind(path_r2.display().to_string())
    .execute(&pool)
    .await
    .expect("publish r2");

    let prior = mr_reviews::load_prior_published_reviews(&pool, project_id, 42)
        .await
        .expect("load prior");
    assert_eq!(prior.len(), 2);
    assert_eq!(prior[0].review_round, 1);
    assert_eq!(prior[0].body, "round-1 published body");
    assert_eq!(prior[1].review_round, 2);
    assert_eq!(prior[1].body, "round-2 published body");

    let empty = mr_reviews::load_prior_published_reviews(&pool, project_id, 7)
        .await
        .expect("empty");
    assert!(empty.is_empty());
}

#[tokio::test]
async fn list_mr_reviews_api_defaults_to_draft() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let draft_dir = temp.path().join("drafts");
    std::fs::create_dir_all(&draft_dir).expect("draft dir");

    for (mr_iid, status) in [(10, "draft"), (11, "draft"), (12, "published")] {
        let draft_path = draft_dir.join(format!("mr-{mr_iid}.md"));
        std::fs::write(
            &draft_path,
            format!(
                "---\nmr_iid: {mr_iid}\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody {mr_iid}\n"
            ),
        )
        .expect("write draft");
        ingest_mr_draft(
            &pool,
            project_id,
            &draft_path,
            Some("sess-x"),
            ReviewerAgent::Cursor,
            false,
        )
        .await
        .expect("ingest");
        if status == "published" {
            sqlx::query("UPDATE mr_reviews SET status = 'published' WHERE draft_md_path = ?")
                .bind(draft_path.display().to_string())
                .execute(&pool)
                .await
                .expect("publish");
        }
    }

    let items = mr_reviews::list_mr_reviews(&pool, None)
        .await
        .expect("list");
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|item| item.status == "draft"));
    assert!(items.iter().all(|item| item.agent_session_id.is_some()));
    assert!(items.iter().all(|item| !item.draft_body.contains("mr_iid:")));
    assert!(items.iter().all(|item| !item.draft_body.trim_start().starts_with("---")));
    assert!(items.iter().any(|item| item.draft_body.contains("body 10")));
}

#[tokio::test]
async fn mr_scan_endpoint_returns_accepted() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/mr-scan"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert!(json["run_id"].as_i64().is_some());
}

#[tokio::test]
async fn mr_scan_returns_409_when_weekly_track_running() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_project', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

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
                .uri(format!("/api/projects/{project_id}/mr-scan"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn mr_scan_force_query_persists_on_run() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/mr-scan?force=1"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let run_id = json["run_id"].as_i64().expect("run_id");

    let force: i64 = sqlx::query_scalar("SELECT mr_scan_force FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("force flag");
    assert_eq!(force, 1);
}

#[tokio::test]
async fn create_manual_mr_scan_run_defaults_force_false() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");
    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_id = reviewer_server::runs::create_manual_mr_scan_run(&pool, project_id, false)
        .await
        .expect("create run");
    let force: i64 = sqlx::query_scalar("SELECT mr_scan_force FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("force");
    assert_eq!(force, 0);
}

#[tokio::test]
async fn published_pending_snippets_only_include_published_reviews() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice Chen')")
        .execute(&pool)
        .await
        .expect("insert person");
    let person_id: i64 = sqlx::query_scalar("SELECT id FROM people WHERE display_name = 'Alice Chen'")
        .fetch_one(&pool)
        .await
        .expect("person id");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");
    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let draft_dir = temp.path().join("drafts");
    std::fs::create_dir_all(&draft_dir).expect("draft dir");

    for (mr_iid, status) in [(10, "published"), (11, "draft")] {
        let draft_path = draft_dir.join(format!("mr-{mr_iid}.md"));
        std::fs::write(
            &draft_path,
            format!(
                "---\nmr_iid: {mr_iid}\nmr_title: t\nreview_round: 1\nauthor_identity: alice@co.com\n---\nbody\n"
            ),
        )
        .expect("write draft");
        ingest_mr_draft(
            &pool,
            project_id,
            &draft_path,
            None,
            ReviewerAgent::Cursor,
            false,
        )
        .await
        .expect("ingest");
        sqlx::query(
            "UPDATE mr_reviews SET person_id = ?, status = ? WHERE mr_iid = ?",
        )
        .bind(person_id)
        .bind(status)
        .bind(mr_iid)
        .execute(&pool)
        .await
        .expect("update status");
    }

    let snippets = mr_reviews::load_published_pending_snippets(&pool, project_id)
        .await
        .expect("snippets");
    assert_eq!(snippets.len(), 1);
    assert_eq!(
        snippets[0],
        "Alice Chen/_pending/mr-10-round-1.md"
    );
}

#[tokio::test]
async fn weekly_manifest_lists_published_pending_snippets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice Chen')")
        .execute(&pool)
        .await
        .expect("insert person");
    let person_id: i64 = sqlx::query_scalar("SELECT id FROM people WHERE display_name = 'Alice Chen'")
        .fetch_one(&pool)
        .await
        .expect("person id");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");
    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let draft_dir = temp.path().join("drafts");
    std::fs::create_dir_all(&draft_dir).expect("draft dir");
    let draft_path = draft_dir.join("mr-42.md");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: alice@co.com\n---\nbody\n",
    )
    .expect("write draft");
    ingest_mr_draft(
        &pool,
        project_id,
        &draft_path,
        None,
        ReviewerAgent::Cursor,
        false,
    )
    .await
    .expect("ingest");
    sqlx::query("UPDATE mr_reviews SET person_id = ?, status = 'published' WHERE mr_iid = 42")
        .bind(person_id)
        .execute(&pool)
        .await
        .expect("publish");

    let project = sqlx::query_as::<_, reviewer_server::runs::ProjectRow>(
        "SELECT id, name, repo_path FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .expect("project");
    let manifest_path = reviewer_server::runs::write_weekly_manifest(
        &pool,
        temp.path(),
        99,
        &project,
        "/tmp/repo",
    )
    .await
    .expect("manifest");

    let json: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("json");
    let snippets = json["published_pending_snippets"]
        .as_array()
        .expect("snippets array");
    assert_eq!(snippets.len(), 1);
    assert_eq!(
        snippets[0].as_str().expect("path"),
        "Alice Chen/_pending/mr-42-round-1.md"
    );
}
