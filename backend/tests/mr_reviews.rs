use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::config::ReviewerAgent;
use reviewer_server::db::{init_pool, table_exists};
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

#[tokio::test]
async fn migration_013_creates_chat_messages_table_and_schema_version() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    assert!(
        table_exists(&pool, "mr_review_chat_messages")
            .await
            .expect("table_exists"),
        "mr_review_chat_messages table must exist after migrations"
    );

    let has_version: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM schema_version WHERE version = 13",
    )
    .fetch_one(&pool)
    .await
    .expect("schema_version");
    assert_eq!(has_version, 1, "schema_version must include 13");

    let index_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_mr_review_chat_messages_review'",
    )
    .fetch_one(&pool)
    .await
    .expect("index");
    assert_eq!(index_count, 1);
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

    mr_reviews::update_draft(&pool, review_id, "# Edited\n\nNo yaml here", None)
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

    let report_root = temp.path().join("reports/alpha");
    let pending = report_root.join("Alice Chen/_pending");
    std::fs::create_dir_all(&pending).expect("pending dir");
    std::fs::write(pending.join("mr-10-round-1.md"), "obs\n").expect("snippet");

    let snippets = mr_reviews::load_published_pending_snippets(&pool, project_id, &report_root)
        .await
        .expect("snippets");
    assert_eq!(snippets.len(), 1);
    assert_eq!(
        snippets[0],
        "Alice Chen/_pending/mr-10-round-1.md"
    );

    // Consumed (deleted) snippets must not be re-listed.
    std::fs::remove_file(pending.join("mr-10-round-1.md")).expect("delete");
    let after = mr_reviews::load_published_pending_snippets(&pool, project_id, &report_root)
        .await
        .expect("snippets after delete");
    assert!(after.is_empty());
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

    let pending = temp
        .path()
        .join("reports/alpha/Alice Chen/_pending");
    std::fs::create_dir_all(&pending).expect("pending dir");
    std::fs::write(pending.join("mr-42-round-1.md"), "obs\n").expect("snippet");

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

fn agent_turn_executor_path(ok: bool) -> std::path::PathBuf {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let name = if ok {
        "agent_turn_ok"
    } else {
        "agent_turn_fail"
    };
    if cfg!(windows) {
        fixtures.join(format!("{name}.cmd"))
    } else {
        fixtures.join(format!("{name}.sh"))
    }
}

fn git_for_test(args: &[&str], cwd: &std::path::Path) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_source_repo(path: &std::path::Path) {
    std::fs::create_dir_all(path).expect("source dir");
    git_for_test(&["init", "-b", "main", "."], path);
    git_for_test(&["config", "user.email", "t@e.com"], path);
    git_for_test(&["config", "user.name", "T"], path);
    std::fs::write(path.join("a.txt"), "a").expect("a.txt");
    git_for_test(&["add", "-A"], path);
    git_for_test(&["commit", "-m", "init"], path);
}

struct AgentTurnFixture {
    temp: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    review_id: i64,
    draft_path: std::path::PathBuf,
}

async fn setup_agent_turn_fixture(executor_ok: bool) -> AgentTurnFixture {
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;
    std::env::set_var("REVIEWER_EXECUTOR", agent_turn_executor_path(executor_ok));

    let source = temp.path().join("source");
    init_source_repo(&source);
    let container = temp.path().join("repos/alpha");
    let source_url = source.display().to_string().replace('\\', "/");
    reviewer_server::worktree::provision_project(
        &container,
        &source_url,
        &["main".to_string()],
    )
    .await
    .expect("provision");

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo, default_branch) VALUES ('alpha', ?, 1, 'main')",
    )
    .bind(container.display().to_string())
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

    let review_id: i64 = sqlx::query_scalar("SELECT id FROM mr_reviews WHERE mr_iid = 42")
        .fetch_one(&pool)
        .await
        .expect("id");

    AgentTurnFixture {
        temp,
        pool,
        review_id,
        draft_path,
    }
}

#[tokio::test]
async fn agent_turn_persists_user_then_assistant_on_success() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let fixture = setup_agent_turn_fixture(true).await;
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let response = mr_reviews::agent_turn(
        &fixture.pool,
        &config,
        fixture.review_id,
        "why flag helper?",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("agent turn");

    assert_eq!(response.reply, "because it wraps commits");
    assert!(!response.draft_hash.is_empty());
    assert!(response.draft_body.contains("First body"));

    let messages = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, role, content FROM mr_review_chat_messages WHERE mr_review_id = ? ORDER BY id ASC",
    )
    .bind(fixture.review_id)
    .fetch_all(&fixture.pool)
    .await
    .expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].1, "user");
    assert_eq!(messages[0].2, "why flag helper?");
    assert_eq!(messages[1].1, "assistant");
    assert_eq!(messages[1].2, "because it wraps commits");
    assert!(messages[1].0 > messages[0].0);

    std::env::remove_var("REVIEWER_EXECUTOR");
    let _ = fixture.temp;
}

#[tokio::test]
async fn agent_turn_failure_leaves_chat_unchanged() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let fixture = setup_agent_turn_fixture(false).await;
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let err = mr_reviews::agent_turn(
        &fixture.pool,
        &config,
        fixture.review_id,
        "why flag helper?",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect_err("agent turn must fail");
    assert!(err.to_string().contains("agent failed") || err.to_string().contains("Agent"), "err={err}");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mr_review_chat_messages WHERE mr_review_id = ?",
    )
    .bind(fixture.review_id)
    .fetch_one(&fixture.pool)
    .await
    .expect("count");
    assert_eq!(count, 0);

    std::env::remove_var("REVIEWER_EXECUTOR");
    let _ = fixture.temp;
}

#[tokio::test]
async fn list_mr_reviews_includes_chat_messages_and_draft_hash() {
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
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody text\n",
    )
    .expect("write");

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

    sqlx::query(
        "INSERT INTO mr_review_chat_messages (mr_review_id, role, content) VALUES (?, 'user', 'why flag helper?')",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("user msg");
    sqlx::query(
        "INSERT INTO mr_review_chat_messages (mr_review_id, role, content) VALUES (?, 'assistant', 'because it wraps commits')",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("assistant msg");

    let items = mr_reviews::list_mr_reviews(&pool, Some("draft"))
        .await
        .expect("list");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].chat_messages.len(), 2);
    assert_eq!(items[0].chat_messages[0].role, "user");
    assert_eq!(items[0].chat_messages[0].content, "why flag helper?");
    assert_eq!(items[0].chat_messages[1].role, "assistant");
    assert_eq!(
        items[0].chat_messages[1].content,
        "because it wraps commits"
    );
    assert!(items[0].chat_messages[1].id > items[0].chat_messages[0].id);
    assert_eq!(
        items[0].draft_hash,
        mr_reviews::draft_body_hash(&items[0].draft_body)
    );
    assert!(!items[0].draft_hash.is_empty());
}

#[tokio::test]
async fn list_mr_reviews_api_embeds_chat_messages_json() {
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

    let draft_path = temp.path().join("drafts/mr-42.md");
    std::fs::create_dir_all(draft_path.parent().expect("parent")).expect("dir");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody text\n",
    )
    .expect("write");
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
    sqlx::query(
        "INSERT INTO mr_review_chat_messages (mr_review_id, role, content) VALUES (?, 'user', 'why flag helper?')",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("user");
    sqlx::query(
        "INSERT INTO mr_review_chat_messages (mr_review_id, role, content) VALUES (?, 'assistant', 'because it wraps commits')",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("assistant");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/mr-reviews?status=draft")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let item = &json.as_array().expect("array")[0];
    let chat = item["chat_messages"].as_array().expect("chat_messages");
    assert_eq!(chat.len(), 2);
    assert_eq!(chat[0]["role"], "user");
    assert_eq!(chat[0]["content"], "why flag helper?");
    assert_eq!(chat[1]["role"], "assistant");
    assert_eq!(chat[1]["content"], "because it wraps commits");
    assert!(item["draft_hash"].as_str().expect("hash").len() == 64);
}

#[tokio::test]
async fn publish_retains_chat_messages_in_published_list() {
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

    let draft_path = temp.path().join("drafts/mr-42.md");
    std::fs::create_dir_all(draft_path.parent().expect("parent")).expect("dir");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody\n",
    )
    .expect("write");
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
    sqlx::query(
        "INSERT INTO mr_review_chat_messages (mr_review_id, role, content) VALUES (?, 'user', 'q')",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("user");
    sqlx::query(
        "INSERT INTO mr_review_chat_messages (mr_review_id, role, content) VALUES (?, 'assistant', 'a')",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("assistant");

    sqlx::query(
        "UPDATE mr_reviews SET status = 'published', published_body = 'body', published_at = datetime('now') WHERE id = ?",
    )
    .bind(review_id)
    .execute(&pool)
    .await
    .expect("publish");

    let published = mr_reviews::list_mr_reviews(&pool, Some("published"))
        .await
        .expect("list published");
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].chat_messages.len(), 2);
    assert_eq!(published[0].chat_messages[0].content, "q");
    assert_eq!(published[0].chat_messages[1].content, "a");

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let err = mr_reviews::agent_turn(
        &pool,
        &config,
        review_id,
        "more?",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect_err("published agent-turn");
    assert!(matches!(err, reviewer_server::Error::MrReviewConflict));
}

#[tokio::test]
async fn agent_turn_returns_reread_draft_when_file_changed() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let fixture = setup_agent_turn_fixture(true).await;
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let mutated = format!(
        "---\nmr_iid: 42\nmr_title: feat cache\nreview_round: 1\nauthor_identity: alice@co.com\n---\nAgent revised body\n"
    );
    std::env::set_var("AGENT_TURN_DRAFT_PATH", &fixture.draft_path);
    std::env::set_var("AGENT_TURN_DRAFT_BODY", &mutated);

    let response = mr_reviews::agent_turn(
        &fixture.pool,
        &config,
        fixture.review_id,
        "please revise",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("agent turn");

    assert!(response.draft_body.contains("Agent revised body"));
    assert_eq!(
        response.draft_hash,
        mr_reviews::draft_body_hash(&response.draft_body)
    );

    std::env::remove_var("AGENT_TURN_DRAFT_PATH");
    std::env::remove_var("AGENT_TURN_DRAFT_BODY");
    std::env::remove_var("REVIEWER_EXECUTOR");
    let _ = fixture.temp;
}

#[tokio::test]
async fn agent_turn_returns_unchanged_draft_body_and_hash() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let fixture = setup_agent_turn_fixture(true).await;
    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    std::env::remove_var("AGENT_TURN_DRAFT_PATH");
    std::env::remove_var("AGENT_TURN_DRAFT_BODY");

    let response = mr_reviews::agent_turn(
        &fixture.pool,
        &config,
        fixture.review_id,
        "why flag helper?",
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("agent turn");

    assert!(response.draft_body.contains("First body"));
    assert_eq!(
        response.draft_hash,
        mr_reviews::draft_body_hash(&response.draft_body)
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
    let _ = fixture.temp;
}

#[tokio::test]
async fn update_draft_rejects_stale_base_hash_without_writing() {
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
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nOn disk body\n",
    )
    .expect("write");
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

    let err = mr_reviews::update_draft(&pool, review_id, "New body", Some("deadbeef"))
        .await
        .expect_err("stale hash");
    match err {
        reviewer_server::Error::DraftBaseHashConflict {
            draft_body,
            draft_hash,
        } => {
            assert!(draft_body.contains("On disk body"));
            assert_eq!(draft_hash, mr_reviews::draft_body_hash(&draft_body));
        }
        other => panic!("expected DraftBaseHashConflict, got {other}"),
    }
    let on_disk = std::fs::read_to_string(&draft_path).expect("read");
    assert!(on_disk.contains("On disk body"));
    assert!(!on_disk.contains("New body"));
}

#[tokio::test]
async fn update_draft_saves_when_base_hash_matches() {
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
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nOn disk body\n",
    )
    .expect("write");
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

    let raw = std::fs::read_to_string(&draft_path).expect("read");
    let body = mr_reviews::strip_draft_frontmatter(&raw);
    let current_hash = mr_reviews::draft_body_hash(body);

    mr_reviews::update_draft(&pool, review_id, "Saved body", Some(&current_hash))
        .await
        .expect("update");
    let on_disk = std::fs::read_to_string(&draft_path).expect("read");
    assert!(on_disk.contains("Saved body"));
    assert!(!on_disk.contains("On disk body"));
}

#[tokio::test]
async fn patch_mr_review_api_returns_409_json_on_stale_base_hash() {
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
    let draft_path = temp.path().join("drafts/mr-42.md");
    std::fs::create_dir_all(draft_path.parent().expect("parent")).expect("dir");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 42\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nDisk\n",
    )
    .expect("write");
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

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/mr-reviews/{review_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"draft_body":"Nope","base_hash":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert!(json["draft_body"].as_str().unwrap().contains("Disk"));
    assert_eq!(
        json["draft_hash"].as_str().unwrap().len(),
        64
    );
    let on_disk = std::fs::read_to_string(&draft_path).expect("read");
    assert!(on_disk.contains("Disk"));
    assert!(!on_disk.contains("Nope"));
}

#[tokio::test]
async fn restore_moves_ignored_review_back_to_draft() {
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

    let draft_path = temp.path().join("drafts/mr-5.md");
    std::fs::create_dir_all(draft_path.parent().expect("parent")).expect("dir");
    std::fs::write(
        &draft_path,
        "---\nmr_iid: 5\nmr_title: t\nreview_round: 1\nauthor_identity: a@b.com\n---\nbody\n",
    )
    .expect("write");
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
    let review_id: i64 = sqlx::query_scalar("SELECT id FROM mr_reviews WHERE mr_iid = 5")
        .fetch_one(&pool)
        .await
        .expect("id");

    mr_reviews::ignore(&pool, review_id).await.expect("ignore");
    let status: String = sqlx::query_scalar("SELECT status FROM mr_reviews WHERE id = ?")
        .bind(review_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "ignored");

    mr_reviews::restore(&pool, review_id).await.expect("restore");
    let status: String = sqlx::query_scalar("SELECT status FROM mr_reviews WHERE id = ?")
        .bind(review_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "draft");

    let err = mr_reviews::restore(&pool, review_id)
        .await
        .expect_err("draft cannot restore");
    assert!(matches!(err, reviewer_server::Error::MrReviewConflict));
}
