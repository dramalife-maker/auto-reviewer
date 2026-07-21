use std::sync::Mutex;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::build_app;
use reviewer_server::db::init_pool;
use reviewer_server::projects::load_from_yaml;
use reviewer_server::runs;
use reviewer_server::summary::{count_pending_for_person, count_reports_for_run, ingest_project_summaries, parse_summary_file};
use reviewer_server::worker::{process_run_project, resolve_working_dir};
use reviewer_server::worktree::provision_all;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_app_state_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

async fn insert_projects(pool: &sqlx::SqlitePool, temp: &tempfile::TempDir) {
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(pool)
    .await
    .expect("insert alpha");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('beta', ?, 0)",
    )
    .bind(temp.path().join("repos/beta").display().to_string())
    .execute(pool)
    .await
    .expect("insert beta");
}

#[tokio::test]
async fn fetch_next_queued_run_project_claims_row_once() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project");

    let first = runs::fetch_next_queued_run_project(&pool)
        .await
        .expect("claim")
        .expect("one job");
    assert_eq!(first.name, "alpha");

    let state: String = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .expect("state");
    assert_eq!(state, "running");

    let second = runs::fetch_next_queued_run_project(&pool)
        .await
        .expect("second claim");
    assert!(
        second.is_none(),
        "same queued row must not be claimed twice"
    );
}

/// Claiming used to run SELECT-then-UPDATE inside one deferred transaction, so
/// a concurrent commit invalidated the read snapshot and the upgrade failed
/// with SQLITE_BUSY_SNAPSHOT ("database is locked") — which busy_timeout never
/// retries. Contract: concurrent claimers alongside a concurrent writer must
/// all succeed and split the queue with no row claimed twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claims_split_queue_without_lock_errors() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    // UNIQUE (run_id, project_id) forces one run per queued row.
    const QUEUED: usize = 12;
    for _ in 0..QUEUED {
        let run_id = sqlx::query(
            "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
        )
        .execute(&pool)
        .await
        .expect("insert run")
        .last_insert_rowid();

        sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')")
            .bind(run_id)
            .bind(project_id)
            .execute(&pool)
            .await
            .expect("insert run project");
    }

    // Keep committing writes to run_projects for the whole claim window; these
    // are what used to poison a claimer's read snapshot.
    let writer_pool = pool.clone();
    let writer_stop = CancellationToken::new();
    let writer_token = writer_stop.clone();
    let writer = tokio::spawn(async move {
        while !writer_token.is_cancelled() {
            sqlx::query("UPDATE run_projects SET error = 'noise' WHERE id = (SELECT MAX(id) FROM run_projects)")
                .execute(&writer_pool)
                .await
                .expect("noise write");
            tokio::task::yield_now().await;
        }
    });

    let mut claimers = Vec::new();
    for _ in 0..8 {
        let claim_pool = pool.clone();
        claimers.push(tokio::spawn(async move {
            let mut claimed = Vec::new();
            loop {
                match runs::fetch_next_queued_run_project(&claim_pool).await {
                    Ok(Some(job)) => claimed.push(job.id),
                    Ok(None) => return Ok(claimed),
                    Err(err) => return Err(err.to_string()),
                }
            }
        }));
    }

    let mut claimed_ids = Vec::new();
    for claimer in claimers {
        let result = claimer.await.expect("claimer task");
        let ids = result.expect("claiming must not fail under concurrent writes");
        claimed_ids.extend(ids);
    }

    writer_stop.cancel();
    writer.await.expect("writer task");

    claimed_ids.sort_unstable();
    let total = claimed_ids.len();
    claimed_ids.dedup();
    assert_eq!(
        claimed_ids.len(),
        total,
        "no queued row may be claimed more than once"
    );
    assert_eq!(total, QUEUED, "every queued row must be claimed exactly once");

    let still_queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM run_projects WHERE state = 'queued'")
            .fetch_one(&pool)
            .await
            .expect("queued count");
    assert_eq!(still_queued, 0, "queue must be fully drained");
}

/// `wake()` only fires when a run is created, so a drain that aborts mid-way
/// would strand `queued` rows — and stranded rows keep `has_active_run_projects`
/// true, rejecting every later run with `RunConflict`. The periodic tick is the
/// safety net that recovers without any `wake()`.
#[tokio::test]
async fn periodic_tick_drains_queue_without_wake() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_project_id = sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project")
    .last_insert_rowid();

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let cancel = CancellationToken::new();
    // Deliberately never call wake(): only the tick can drain this row.
    let _worker = reviewer_server::worker::RunWorker::spawn_with_tick(
        config,
        pool.clone(),
        cancel.clone(),
        std::time::Duration::from_millis(50),
    );

    let mut state = String::from("queued");
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        state = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
            .bind(run_project_id)
            .fetch_one(&pool)
            .await
            .expect("state");
        if state != "queued" {
            break;
        }
    }
    cancel.cancel();

    assert_ne!(
        state, "queued",
        "the periodic tick must drain queued rows even when wake() is never called"
    );
}

#[tokio::test]
async fn drain_queue_does_not_dequeue_after_cancel() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_project_id = sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project")
    .last_insert_rowid();

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let cancel = CancellationToken::new();
    cancel.cancel();
    let worker = reviewer_server::worker::RunWorker::spawn(config, pool.clone(), cancel);

    worker.drain_queue().await.expect("drain queue");

    let state: String = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(run_project_id)
        .fetch_one(&pool)
        .await
        .expect("state");
    assert_eq!(
        state, "queued",
        "drain_queue must not dequeue after the worker's token is cancelled"
    );
}

#[tokio::test]
async fn manual_all_run_enqueues_projects() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"trigger":"manual_all"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let run_id = json["run_id"].as_i64().expect("run_id");

    assert_eq!(
        runs::count_run_projects_by_state(&pool, run_id, "queued")
            .await
            .expect("count"),
        2
    );
}

#[tokio::test]
async fn manual_project_run_enqueues_one_project() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"trigger":"manual_project","project_name":"alpha"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let run_id = json["run_id"].as_i64().expect("run_id");

    assert_eq!(
        runs::count_run_projects_by_state(&pool, run_id, "queued")
            .await
            .expect("count"),
        1
    );

    let trigger: String = sqlx::query_scalar("SELECT trigger FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("trigger");
    assert_eq!(trigger, "manual_project");

    let project_name: String = sqlx::query_scalar(
        "SELECT p.name FROM run_projects rp
         INNER JOIN projects p ON p.id = rp.project_id
         WHERE rp.run_id = ?",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("project name");
    assert_eq!(project_name, "alpha");
}

#[tokio::test]
async fn duplicate_project_run_returns_409() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    insert_projects(&pool, &temp).await;

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");

    let run_id = run_result.last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

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
                .uri("/api/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"trigger":"manual_all"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn startup_recovery_fails_orphaned_running_project_and_finalizes_run() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let run_project_id = sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'running')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project")
    .last_insert_rowid();

    runs::recover_orphaned_running_projects(&pool)
        .await
        .expect("recover orphaned running projects");

    let (state, error): (String, Option<String>) = sqlx::query_as(
        "SELECT state, error FROM run_projects WHERE id = ?",
    )
    .bind(run_project_id)
    .fetch_one(&pool)
    .await
    .expect("run project row");
    assert_eq!(state, "failed");
    assert!(
        error
            .as_deref()
            .unwrap_or_default()
            .contains("interrupted by previous shutdown"),
        "error={error:?}"
    );

    let run_status: String = sqlx::query_scalar("SELECT status FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("run status");
    assert_eq!(run_status, "failed", "parent run must be finalized");
}

#[tokio::test]
async fn startup_recovery_leaves_queued_rows_untouched() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let run_project_id = sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project")
    .last_insert_rowid();

    runs::recover_orphaned_running_projects(&pool)
        .await
        .expect("recover orphaned running projects");

    let state: String = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(run_project_id)
        .fetch_one(&pool)
        .await
        .expect("state");
    assert_eq!(state, "queued", "queued rows must survive startup recovery");
}

#[tokio::test]
async fn worker_marks_skipped_timeout() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let slow_executor = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/slow_executor.cmd");
    std::env::set_var("REVIEWER_EXECUTOR", &slow_executor);

    let pool = init_pool(temp.path()).await.expect("init pool");
    std::fs::create_dir_all(temp.path().join("repos/alpha")).expect("repo dir");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    sqlx::query("UPDATE schedule_config SET per_project_timeout_sec = 1 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("update timeout");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("project id");

    let run_project_result = sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'queued')",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("insert run project");

    let run_project_id = run_project_result.last_insert_rowid();

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let job = runs::RunProjectRow {
        id: run_project_id,
        run_id,
        project_id,
        name: "alpha".into(),
        repo_path: temp.path().join("repos/alpha").display().to_string(),
        trigger: "manual_project".into(),
        mr_scan_force: 0,
    };

    process_run_project(&pool, &config, job, 1, CancellationToken::new())
        .await
        .expect("process run project");

    let state: String = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(run_project_id)
        .fetch_one(&pool)
        .await
        .expect("state");

    assert_eq!(state, "skipped_timeout");

    std::env::remove_var("REVIEWER_EXECUTOR");
}

#[tokio::test]
async fn mr_scan_timeout_still_ingests_draft_on_disk() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let triage = fixtures.join("fake_triage_eligible.py");
    let executor = if cfg!(windows) {
        fixtures.join("write_draft_then_hang.cmd")
    } else {
        fixtures.join("write_draft_then_hang.sh")
    };
    std::env::set_var("REVIEWER_TRIAGE_SCRIPT", &triage);
    std::env::set_var("REVIEWER_EXECUTOR", &executor);
    std::env::set_var("REVIEWER_TEST_MR_IID", "68");

    let pool = init_pool(temp.path()).await.expect("init pool");
    std::fs::create_dir_all(temp.path().join("repos/alpha")).expect("repo dir");

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

    let run_id = runs::create_manual_mr_scan_run(&pool, project_id, false)
        .await
        .expect("create mr scan run");

    let draft_path = runs::mr_poll_draft_dir(temp.path(), run_id, project_id).join("mr-68-round-1.md");
    std::env::set_var("REVIEWER_TEST_DRAFT_FILE", &draft_path);

    let run_project_id: i64 = sqlx::query_scalar(
        "SELECT id FROM run_projects WHERE run_id = ? AND project_id = ?",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .expect("run project id");

    // Rebuild config after env vars are set (executor + triage).
    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let job = runs::RunProjectRow {
        id: run_project_id,
        run_id,
        project_id,
        name: "alpha".into(),
        repo_path: temp.path().join("repos/alpha").display().to_string(),
        trigger: "manual_mr_poll".into(),
        mr_scan_force: 0,
    };

    process_run_project(&pool, &config, job, 1, CancellationToken::new())
        .await
        .expect("process mr scan");

    let state: String = sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(run_project_id)
        .fetch_one(&pool)
        .await
        .expect("state");
    assert_eq!(state, "skipped_timeout");

    assert!(draft_path.is_file(), "executor should have written draft before hang");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mr_reviews WHERE project_id = ? AND mr_iid = 68 AND status = 'draft'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .expect("count drafts");
    assert_eq!(count, 1, "timeout path must still ingest on-disk drafts");

    std::env::remove_var("REVIEWER_EXECUTOR");
    std::env::remove_var("REVIEWER_TRIAGE_SCRIPT");
    std::env::remove_var("REVIEWER_TEST_DRAFT_FILE");
    std::env::remove_var("REVIEWER_TEST_MR_IID");
}

fn init_source_repo(path: &std::path::Path) {
    use std::process::Command;
    std::fs::create_dir_all(path).expect("source dir");
    let p = path.display().to_string();
    for args in [
        vec!["init", "-b", "main", &p],
        vec!["-C", &p, "config", "user.email", "t@e.com"],
        vec!["-C", &p, "config", "user.name", "T"],
        vec!["-C", &p, "commit", "--allow-empty", "-m", "init"],
    ] {
        let out = Command::new("git").args(&args).output().expect("git");
        assert!(out.status.success(), "git {args:?}");
    }
}

#[tokio::test]
async fn resolve_working_dir_returns_resident_worktree() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    std::env::remove_var("REVIEWER_EXECUTOR");
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    init_source_repo(&source);
    let container = temp.path().join("repos/svc");
    let container_display = container.display().to_string().replace('\\', "/");
    let source_url = source.display().to_string().replace('\\', "/");

    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(
        &yaml_path,
        format!(
            "projects:\n  - name: svc\n    repo_path: {container_display}\n    git_remote_url: {source_url}\n    default_branches:\n      - main\n"
        ),
    )
    .expect("write yaml");

    let pool = init_pool(temp.path()).await.expect("init pool");
    let resolved = load_from_yaml(&pool, temp.path(), &yaml_path).await.expect("load");
    provision_all(&pool, &resolved).await;

    let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'svc'")
        .fetch_one(&pool)
        .await
        .expect("project id");
    let job = runs::RunProjectRow {
        id: 1,
        run_id: 1,
        project_id,
        name: "svc".into(),
        repo_path: container.display().to_string(),
        trigger: "manual_project".into(),
        mr_scan_force: 0,
    };

    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let config = reviewer_server::config::AppConfig::from_env().expect("config");

    let dir = resolve_working_dir(&pool, &config, &job)
        .await
        .expect("resolve dir");
    assert_eq!(dir, container.join("main"), "resident worktree path");

    // An unhealthy / unprovisioned project cannot supply a worktree.
    let bad_job = runs::RunProjectRow {
        id: 2,
        run_id: 1,
        project_id: 999,
        name: "missing".into(),
        repo_path: container.display().to_string(),
        trigger: "manual_project".into(),
        mr_scan_force: 0,
    };
    assert!(resolve_working_dir(&pool, &config, &bad_job).await.is_err());
}

#[tokio::test]
async fn summary_parser_creates_report_and_pending() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
        .execute(&pool)
        .await
        .expect("insert person");

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
mr_count: 6
commit_count: 42
---

## 待確認
- First question?
- Second question?
"#,
    )
    .expect("write summary");

    let parsed = parse_summary_file(&summary_path).expect("parse summary");
    assert_eq!(parsed.pending_questions.len(), 2);
    assert!(parsed.resolved_questions.is_empty());

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    ingest_project_summaries(
        &pool,
        temp.path(),
        "game-backend",
        1,
        run_id,
    )
    .await
    .expect("ingest summaries");

    assert_eq!(
        count_reports_for_run(&pool, run_id).await.expect("reports"),
        1
    );
    assert_eq!(
        count_pending_for_person(&pool, "Alice")
            .await
            .expect("pending"),
        2
    );

    let one_line: String = sqlx::query_scalar(
        "SELECT one_line FROM reports WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("one_line");
    assert_eq!(one_line, "Stable week");
}


#[tokio::test]
async fn duplicate_open_question_is_not_inserted_again() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
        .execute(&pool)
        .await
        .expect("insert person");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    for date in ["2026-07-05", "2026-07-12"] {
        let summary_path = temp
            .path()
            .join(format!("reports/game-backend/Alice/{date}/summary.md"));
        std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(
            &summary_path,
            format!(
                r#"---
person: Alice
project: game-backend
date: {date}
one_line: Stable week
---

## 待確認
- Why choose A?
"#
            ),
        )
        .expect("write summary");

        let run_result = sqlx::query(
            "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
        )
        .execute(&pool)
        .await
        .expect("insert run");
        let run_id = run_result.last_insert_rowid();

        ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
            .await
            .expect("ingest summaries");
    }

    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = 'Why choose A?' AND status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(open_count, 1);
}

#[tokio::test]
async fn summary_parser_reads_resolved_section() {
    let temp = tempfile::tempdir().expect("tempdir");
    let summary_path = temp.path().join("summary.md");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice Chen
project: game-backend
date: 2026-07-05
one_line: Cleared one item
---

## 本週重點
- Shipping

## 成長面向
- Clarity

## 待確認
- Still open?

## 已釐清
- Why choose A?
"#,
    )
    .expect("write summary");

    let parsed = parse_summary_file(&summary_path).expect("parse summary");
    assert_eq!(parsed.pending_questions, vec!["Still open?".to_string()]);
    assert_eq!(parsed.resolved_questions, vec!["Why choose A?".to_string()]);
}

#[tokio::test]
async fn ingest_resolved_section_closes_open_pending() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(&pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let open_id = sqlx::query(
        "INSERT INTO pending_items (person_id, project_id, question, status, raised_date)
         VALUES (?, 1, 'Why choose A?', 'open', '2026-07')",
    )
    .bind(person_id)
    .execute(&pool)
    .await
    .expect("insert open")
    .last_insert_rowid();

    let notes_path = temp.path().join("reports/_people/Alice/_notes.md");
    std::fs::create_dir_all(notes_path.parent().expect("parent")).expect("mkdir notes");
    std::fs::write(&notes_path, "- [2026-07] Why choose A?\n").expect("write notes");

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-12/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-12
one_line: Cleared
---

## 待確認

## 已釐清
- Why choose A?
"#,
    )
    .expect("write summary");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest");

    let status: String = sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
        .bind(open_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "resolved");

    let resolved_date: Option<String> =
        sqlx::query_scalar("SELECT resolved_date FROM pending_items WHERE id = ?")
            .bind(open_id)
            .fetch_one(&pool)
            .await
            .expect("resolved_date");
    assert!(resolved_date.is_some());
    assert_eq!(resolved_date.as_deref().unwrap().len(), 7);

    let notes = std::fs::read_to_string(&notes_path).expect("read notes");
    assert!(notes.contains("✓ Why choose A?"), "notes={notes}");
}

#[tokio::test]
async fn ingest_omission_without_resolved_keeps_open() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(&pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let open_id = sqlx::query(
        "INSERT INTO pending_items (person_id, project_id, question, status, raised_date)
         VALUES (?, 1, 'Why choose A?', 'open', '2026-07')",
    )
    .bind(person_id)
    .execute(&pool)
    .await
    .expect("insert open")
    .last_insert_rowid();

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-12/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-12
one_line: No pending this week
---

## 待確認

## 已釐清
"#,
    )
    .expect("write summary");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest");

    let status: String = sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
        .bind(open_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "open");
}

#[tokio::test]
async fn ingest_unknown_resolved_bullet_is_ignored() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
        .execute(&pool)
        .await
        .expect("insert person");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-12/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-12
one_line: Noise
---

## 待確認

## 已釐清
- Never seen?
"#,
    )
    .expect("write summary");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = 'Never seen?'",
    )
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn ingest_notes_sync_failure_keeps_pending_resolved() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(&pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let open_id = sqlx::query(
        "INSERT INTO pending_items (person_id, project_id, question, status, raised_date)
         VALUES (?, 1, 'Why choose A?', 'open', '2026-07')",
    )
    .bind(person_id)
    .execute(&pool)
    .await
    .expect("insert open")
    .last_insert_rowid();

    // Block notes sync: make the person trends path a file so create_dir_all fails.
    let people_root = temp.path().join("reports/_people");
    std::fs::create_dir_all(&people_root).expect("mkdir _people");
    std::fs::write(people_root.join("Alice"), "not a directory").expect("block notes path");

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-12/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-12
one_line: Cleared
---

## 待確認

## 已釐清
- Why choose A?
"#,
    )
    .expect("write summary");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest continues despite notes failure");

    let status: String = sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
        .bind(open_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "resolved");
}

#[tokio::test]
async fn ingest_dual_section_still_resolves_open() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(&pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let open_id = sqlx::query(
        "INSERT INTO pending_items (person_id, project_id, question, status, raised_date)
         VALUES (?, 1, 'Why choose A?', 'open', '2026-07')",
    )
    .bind(person_id)
    .execute(&pool)
    .await
    .expect("insert open")
    .last_insert_rowid();

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-12/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-12
one_line: Dual
---

## 待確認
- Why choose A?

## 已釐清
- Why choose A?
"#,
    )
    .expect("write summary");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest");

    let status: String = sqlx::query_scalar("SELECT status FROM pending_items WHERE id = ?")
        .bind(open_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "resolved");
}

#[tokio::test]
async fn resolved_question_may_be_raised_again() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let person_id: i64 = {
        let result = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
            .execute(&pool)
            .await
            .expect("insert person");
        result.last_insert_rowid()
    };

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    sqlx::query(
        "INSERT INTO pending_items (person_id, project_id, question, status, raised_date, resolved_date)
         VALUES (?, 1, 'Why choose A?', 'resolved', '2026-06', '2026-07')",
    )
    .bind(person_id)
    .execute(&pool)
    .await
    .expect("insert resolved pending item");

    let summary_path = temp
        .path()
        .join("reports/game-backend/Alice/2026-07-12/summary.md");
    std::fs::create_dir_all(summary_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &summary_path,
        r#"---
person: Alice
project: game-backend
date: 2026-07-12
one_line: Stable week
---

## 待確認
- Why choose A?
"#,
    )
    .expect("write summary");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let run_id = run_result.last_insert_rowid();

    ingest_project_summaries(&pool, temp.path(), "game-backend", 1, run_id)
        .await
        .expect("ingest summaries");

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_items WHERE question = 'Why choose A?'")
            .fetch_one(&pool)
            .await
            .expect("count");
    assert_eq!(total, 2);

    let open_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_items WHERE question = 'Why choose A?' AND status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .expect("open count");
    assert_eq!(open_count, 1);
}

#[tokio::test]
async fn list_runs_returns_newest_first() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    for (trigger, started) in [
        ("manual_all", "2026-07-01 09:00:00"),
        ("mr_poll", "2026-07-03 09:00:00"),
        ("schedule", "2026-07-02 09:00:00"),
    ] {
        sqlx::query(
            "INSERT INTO runs (trigger, status, started_at, finished_at, duration_sec, project_total, project_skipped)
             VALUES (?, 'success', ?, ?, 60, 1, 0)",
        )
        .bind(trigger)
        .bind(started)
        .bind(started)
        .execute(&pool)
        .await
        .expect("insert run");
    }

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["total"], 3);
    let runs = json["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0]["trigger"], "mr_poll");
    assert_eq!(runs[1]["trigger"], "schedule");
    assert_eq!(runs[2]["trigger"], "manual_all");
    assert_eq!(runs[0]["duration_sec"], 60);
    assert!(runs[0]["id"].is_number());
    assert!(runs[0]["status"].is_string());
    assert!(runs[0]["started_at"].is_string());
}

#[tokio::test]
async fn list_runs_limit_and_offset_paginate() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    for i in 0..3 {
        sqlx::query(
            "INSERT INTO runs (trigger, status, started_at)
             VALUES ('manual_all', 'success', ?)",
        )
        .bind(format!("2026-07-0{} 09:00:00", i + 1))
        .execute(&pool)
        .await
        .expect("insert run");
    }

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs?limit=1&offset=1")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["total"], 3);
    assert_eq!(json["runs"].as_array().expect("runs").len(), 1);
    assert_eq!(json["runs"][0]["started_at"], "2026-07-02 09:00:00");
}

#[tokio::test]
async fn list_runs_filter_by_trigger() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO runs (trigger, status, started_at) VALUES ('mr_poll', 'success', '2026-07-03 09:00:00')",
    )
    .execute(&pool)
    .await
    .expect("insert mr_poll");
    sqlx::query(
        "INSERT INTO runs (trigger, status, started_at) VALUES ('manual_all', 'success', '2026-07-02 09:00:00')",
    )
    .execute(&pool)
    .await
    .expect("insert manual_all");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs?trigger=mr_poll")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.expect("body").to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    let runs = json["runs"].as_array().expect("runs");
    assert!(!runs.is_empty());
    for run in runs {
        assert_eq!(run["trigger"], "mr_poll");
    }
}

#[tokio::test]
async fn list_runs_invalid_limit_returns_400() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs?limit=0")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_runs_limit_over_max_returns_400() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;
    let _pool = init_pool(temp.path()).await.expect("init pool");

    let app = build_app().await.expect("build app");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs?limit=201")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_run_detail_includes_project_error_and_duration() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, duration_sec, project_total, note)
         VALUES ('manual_all', 'failed', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 300, 1, 'batch note')",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state, started_at, finished_at, duration_sec, error)
         VALUES (?, 1, 'failed', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 300, 'agent crashed')",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("insert run_project");

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
    assert_eq!(json["duration_sec"], 300);
    assert_eq!(json["note"], "batch note");
    assert!(json["projects"][0]["skip_summary"].is_null());
    let project = &json["projects"][0];
    assert_eq!(project["name"], "game-backend");
    assert_eq!(project["state"], "failed");
    assert_eq!(project["error"], "agent crashed");
    assert_eq!(project["duration_sec"], 300);
    assert_eq!(project["started_at"], "2026-07-05 09:00:00");
    assert_eq!(project["finished_at"], "2026-07-05 09:05:00");
}

#[tokio::test]
async fn get_run_mr_exposes_skip_summary_from_eligible_file() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, project_total)
         VALUES ('mr_poll', 'success', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state, duration_sec)
         VALUES (?, 1, 'done', 120)",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("insert run_project");

    let eligible_path = runs::eligible_mrs_path(temp.path(), run_id, 1);
    std::fs::create_dir_all(eligible_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &eligible_path,
        r#"{
          "eligible": [],
          "skipped": [
            {"mr_iid": 12, "skip_reason": "inbox_draft"},
            {"mr_iid": 8, "skip_reason": "gitlab_draft"}
          ]
        }"#,
    )
    .expect("write eligible");

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
    let summary = &json["projects"][0]["skip_summary"];
    assert_eq!(summary["by_reason"]["inbox_draft"], 1);
    assert_eq!(summary["by_reason"]["gitlab_draft"], 1);
    let items = summary["items"].as_array().expect("items");
    assert_eq!(items.len(), 2);
    let iids: Vec<i64> = items
        .iter()
        .map(|item| item["mr_iid"].as_i64().expect("mr_iid"))
        .collect();
    assert!(iids.contains(&12));
    assert!(iids.contains(&8));
}

#[tokio::test]
async fn get_run_mr_missing_eligible_file_yields_empty_skip_summary() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, project_total)
         VALUES ('mr_poll', 'success', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'done')",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("insert run_project");

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
    let summary = &json["projects"][0]["skip_summary"];
    assert!(summary["by_reason"].as_object().expect("by_reason").is_empty());
    assert_eq!(summary["items"].as_array().expect("items").len(), 0);
}

#[tokio::test]
async fn get_run_mr_skip_summary_items_capped_at_100() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, project_total)
         VALUES ('mr_poll', 'success', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'done')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert run_project");

    let skipped: Vec<serde_json::Value> = (1..=101)
        .map(|iid| {
            serde_json::json!({
                "mr_iid": iid,
                "skip_reason": "inbox_draft"
            })
        })
        .collect();
    let eligible_path = runs::eligible_mrs_path(temp.path(), run_id, 1);
    std::fs::create_dir_all(eligible_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &eligible_path,
        serde_json::json!({ "eligible": [], "skipped": skipped }).to_string(),
    )
    .expect("write eligible");

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
    let summary = &json["projects"][0]["skip_summary"];
    assert_eq!(summary["by_reason"]["inbox_draft"], 101);
    assert_eq!(summary["items"].as_array().expect("items").len(), 100);
}

#[tokio::test]
async fn get_run_running_mr_omits_skip_summary() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, project_total)
         VALUES ('mr_poll', 'running', '2026-07-05 09:00:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'running')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert run_project");

    let eligible_path = runs::eligible_mrs_path(temp.path(), run_id, 1);
    std::fs::create_dir_all(eligible_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &eligible_path,
        r#"{"eligible":[],"skipped":[{"mr_iid":1,"skip_reason":"inbox_draft"}]}"#,
    )
    .expect("write eligible");

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
    assert!(json["projects"][0]["skip_summary"].is_null());
}

#[tokio::test]
async fn get_run_finished_mr_exposes_draft_outputs_count() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, project_total)
         VALUES ('mr_poll', 'success', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'done')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert run_project");

    let drafts = runs::mr_poll_draft_dir(temp.path(), run_id, 1);
    std::fs::create_dir_all(&drafts).expect("mkdir");
    std::fs::write(drafts.join("68-r1.md"), "# draft").expect("write draft");
    std::fs::write(drafts.join("69-r1.md"), "# draft2").expect("write draft2");

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
    let outputs = &json["projects"][0]["outputs"];
    assert_eq!(outputs["mr_drafts"]["count"], 2);
    assert!(outputs["weekly_reports"].is_null());
}

#[tokio::test]
async fn get_run_finished_weekly_exposes_report_people() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");
    let person_id = sqlx::query("INSERT INTO people (display_name) VALUES ('Alice')")
        .execute(&pool)
        .await
        .expect("insert person")
        .last_insert_rowid();

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, project_total)
         VALUES ('schedule', 'success', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'done')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert run_project");

    sqlx::query(
        "INSERT INTO reports (project_id, person_id, run_id, report_date, report_md_path, summary_md_path)
         VALUES (1, ?, ?, '2026-07-05', 'r.md', 's.md')",
    )
    .bind(person_id)
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("insert report");

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
    let outputs = &json["projects"][0]["outputs"];
    assert!(outputs["mr_drafts"].is_null());
    let people = outputs["weekly_reports"]["people"].as_array().expect("people");
    assert_eq!(people.len(), 1);
    assert_eq!(people[0]["person_id"], person_id);
    assert_eq!(people[0]["display_name"], "Alice");
}

#[tokio::test]
async fn get_run_running_omits_outputs() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, project_total)
         VALUES ('mr_poll', 'running', '2026-07-05 09:00:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'running')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert run_project");

    let drafts = runs::mr_poll_draft_dir(temp.path(), run_id, 1);
    std::fs::create_dir_all(&drafts).expect("mkdir");
    std::fs::write(drafts.join("1.md"), "# x").expect("write");

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
    assert!(json["projects"][0]["outputs"].is_null());
}

#[tokio::test]
async fn get_run_missing_drafts_dir_omits_mr_drafts() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_app_state_env(&temp).await;

    let pool = init_pool(temp.path()).await.expect("init pool");
    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('game-backend', ?, 0)",
    )
    .bind(temp.path().join("repos/game-backend").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, started_at, finished_at, project_total)
         VALUES ('mr_poll', 'success', '2026-07-05 09:00:00', '2026-07-05 09:05:00', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    sqlx::query("INSERT INTO run_projects (run_id, project_id, state) VALUES (?, 1, 'done')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("insert run_project");

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
    assert!(json["projects"][0]["outputs"].is_null());
}
