//! User-initiated run cancellation: per-run tokens, terminal `cancelled`
//! state, the cancel API, output preservation, and shutdown disambiguation.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use reviewer_server::db::init_pool;
use reviewer_server::runs::{self, fetch_next_queued_run_project, RunProjectRow};
use reviewer_server::state::AppState;
use reviewer_server::worker::{process_run_project, RunWorker};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

fn slow_executor_path() -> std::path::PathBuf {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if cfg!(windows) {
        fixtures.join("slow_executor.cmd")
    } else {
        fixtures.join("slow_executor.sh")
    }
}

async fn seed_project(pool: &sqlx::SqlitePool, name: &str, repo_path: &str) -> i64 {
    sqlx::query("INSERT INTO projects (name, repo_path, is_git_repo) VALUES (?, ?, 0)")
        .bind(name)
        .bind(repo_path)
        .execute(pool)
        .await
        .expect("insert project");
    sqlx::query_scalar("SELECT id FROM projects WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("project id")
}

async fn seed_run(pool: &sqlx::SqlitePool, trigger: &str, status: &str) -> i64 {
    sqlx::query(
        "INSERT INTO runs (trigger, status, project_total, started_at)
         VALUES (?, ?, 1, datetime('now'))",
    )
    .bind(trigger)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert run")
    .last_insert_rowid()
}

async fn seed_run_project(
    pool: &sqlx::SqlitePool,
    run_id: i64,
    project_id: i64,
    state: &str,
) -> i64 {
    let started = if state == "queued" { None } else { Some("now") };
    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state, started_at)
         VALUES (?, ?, ?, CASE WHEN ? IS NULL THEN NULL ELSE datetime('now') END)",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(state)
    .bind(started)
    .execute(pool)
    .await
    .expect("insert run_project")
    .last_insert_rowid()
}

async fn rp_state(pool: &sqlx::SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT state FROM run_projects WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("state")
}

async fn rp_started_at(pool: &sqlx::SqlitePool, id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT started_at FROM run_projects WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("started_at")
}

async fn run_status(pool: &sqlx::SqlitePool, run_id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("run status")
}

fn test_config() -> reviewer_server::config::AppConfig {
    reviewer_server::config::AppConfig::from_env().expect("config")
}

// ---------------------------------------------------------------------------
// Task 1.1 — per-run tokens derive from the shutdown token
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shutdown_propagates_through_per_run_tokens() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let pool = init_pool(temp.path()).await.expect("init pool");

    let shutdown = CancellationToken::new();
    let worker = RunWorker::spawn(test_config(), pool, shutdown.clone());

    let run_token = worker.run_token(1);
    assert!(!run_token.is_cancelled());

    shutdown.cancel();

    assert!(
        run_token.is_cancelled(),
        "shutdown must propagate through the per-run child token"
    );
}

#[tokio::test]
async fn cancelling_one_run_leaves_shutdown_and_other_run_intact() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let pool = init_pool(temp.path()).await.expect("init pool");

    let shutdown = CancellationToken::new();
    let worker = RunWorker::spawn(test_config(), pool, shutdown.clone());

    let token_a = worker.run_token(1);
    let token_b = worker.run_token(2);

    worker.cancel_run_token(1);

    assert!(token_a.is_cancelled(), "cancelled run's token must fire");
    assert!(!token_b.is_cancelled(), "other run's token must be intact");
    assert!(!shutdown.is_cancelled(), "shutdown token must be intact");
}

// ---------------------------------------------------------------------------
// Task 1.2 — tokens released when a run ends (both paths)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_token_released_after_normal_completion() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    // A bogus executor path makes the weekly batch fail fast: the run reaches a
    // terminal (failed) status without cancellation — a normal completion.
    std::env::set_var("REVIEWER_EXECUTOR", temp.path().join("does-not-exist"));

    let pool = init_pool(temp.path()).await.expect("init pool");
    let project_id = seed_project(&pool, "alpha", &temp.path().display().to_string()).await;
    let run_id = seed_run(&pool, "manual_all", "running").await;
    seed_run_project(&pool, run_id, project_id, "queued").await;

    let shutdown = CancellationToken::new();
    let worker = RunWorker::spawn(test_config(), pool.clone(), shutdown);
    worker.drain_queue().await.expect("drain");

    assert_ne!(run_status(&pool, run_id).await, "running");
    assert!(
        !worker.run_token_registered(run_id),
        "token must be released once the run is terminal"
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
}

#[tokio::test]
async fn run_token_released_after_cancellation() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    std::env::set_var("REVIEWER_EXECUTOR", slow_executor_path());

    let pool = init_pool(temp.path()).await.expect("init pool");
    let project_id = seed_project(&pool, "alpha", &temp.path().display().to_string()).await;
    let run_id = seed_run(&pool, "manual_all", "running").await;
    seed_run_project(&pool, run_id, project_id, "queued").await;

    let shutdown = CancellationToken::new();
    let worker = RunWorker::spawn(test_config(), pool.clone(), shutdown);

    let drain_worker = worker.clone();
    let drain = tokio::spawn(async move { drain_worker.drain_queue().await });

    // Let the slow executor start, then cancel like the API would.
    tokio::time::sleep(Duration::from_millis(300)).await;
    runs::cancel_run(&pool, run_id).await.expect("cancel run");
    worker.cancel_run_token(run_id);

    drain.await.expect("join drain").expect("drain result");

    assert_eq!(run_status(&pool, run_id).await, "cancelled");
    assert!(
        !worker.run_token_registered(run_id),
        "token must be released once the cancelled run is terminal"
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
}

// ---------------------------------------------------------------------------
// Task 1.3 — cancelled runs are not claimed for execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancelled_run_is_not_claimed() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let project_id = seed_project(&pool, "alpha", "/tmp/alpha").await;
    let run_id = seed_run(&pool, "manual_all", "cancelled").await;
    seed_run_project(&pool, run_id, project_id, "queued").await;

    let claimed = fetch_next_queued_run_project(&pool).await.expect("claim");
    assert!(
        claimed.is_none(),
        "a queued project under a cancelled run must not be claimed"
    );
}

// ---------------------------------------------------------------------------
// Task 2.4 — finalization preserves cancelled status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn finalize_preserves_cancelled_status() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let project_id = seed_project(&pool, "alpha", "/tmp/alpha").await;
    let run_id = seed_run(&pool, "manual_all", "cancelled").await;
    // A late-finishing project that succeeded would normally push the run to
    // `success`; the guard must keep it `cancelled`.
    let rp = seed_run_project(&pool, run_id, project_id, "done").await;
    let _ = rp;

    runs::finalize_run_if_complete(&pool, run_id)
        .await
        .expect("finalize");

    assert_eq!(run_status(&pool, run_id).await, "cancelled");
}

// ---------------------------------------------------------------------------
// Task 2.1 / 2.2 — cancellation source determines terminal state; in-flight
// work is terminated without waiting for the timeout
// ---------------------------------------------------------------------------

async fn run_weekly_with_cancel(cancel_shutdown: bool) -> (String, Duration) {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    std::env::set_var("REVIEWER_EXECUTOR", slow_executor_path());

    let pool = init_pool(temp.path()).await.expect("init pool");
    let repo = temp.path().join("repos/alpha");
    std::fs::create_dir_all(&repo).expect("repo dir");
    let project_id = seed_project(&pool, "alpha", &repo.display().to_string()).await;
    let run_id = seed_run(&pool, "manual_all", "running").await;
    let rp_id = seed_run_project(&pool, run_id, project_id, "running").await;

    let config = test_config();
    let job = RunProjectRow { id: rp_id,
    run_id,
    project_id,
    name: "alpha".into(),
    repo_path: repo.display().to_string(),
    trigger: "manual_all".into(),
    mr_scan_force: 0, person_id: None };

    // run_token is a child of shutdown, mirroring the worker registry.
    let shutdown = CancellationToken::new();
    let run_token = shutdown.child_token();

    let fire = if cancel_shutdown {
        shutdown.clone()
    } else {
        run_token.clone()
    };
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        fire.cancel();
    });

    let started = Instant::now();
    // A 30s timeout the slow executor would otherwise reach; cancellation must
    // win far sooner.
    process_run_project(&pool, &config, job, 30, run_token, shutdown)
        .await
        .expect("process run project");
    let elapsed = started.elapsed();

    let state = rp_state(&pool, rp_id).await;
    std::env::remove_var("REVIEWER_EXECUTOR");
    (state, elapsed)
}

#[tokio::test]
async fn user_cancellation_marks_project_cancelled_quickly() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let (state, elapsed) = run_weekly_with_cancel(false).await;
    assert_eq!(state, "cancelled");
    assert!(
        elapsed < Duration::from_secs(10),
        "cancellation must terminate work well before the 30s timeout (took {elapsed:?})"
    );
}

#[tokio::test]
async fn shutdown_cancellation_marks_project_failed() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let (state, _elapsed) = run_weekly_with_cancel(true).await;
    assert_eq!(
        state, "failed",
        "process shutdown must keep the failed terminal state, not cancelled"
    );
    // The error string is the shutdown marker, distinguishing it from a user cancel.
}

// ---------------------------------------------------------------------------
// Task 3.1 / 2.3 — cancel API endpoint and queued-project handling
// ---------------------------------------------------------------------------

async fn router_with_pool(temp: &tempfile::TempDir, pool: sqlx::SqlitePool) -> axum::Router {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml = temp.path().join("projects.yaml");
    std::fs::write(&yaml, "projects: []\n").expect("yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml);
    let state = AppState {
        config: test_config(),
        pool,
        worker: None,
        shutdown: CancellationToken::new(),
    };
    reviewer_server::server::router(state)
}

async fn post_cancel(app: axum::Router, run_id: i64) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/runs/{run_id}/cancel"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn cancel_api_cancels_running_run_and_queued_projects() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let project_id = seed_project(&pool, "alpha", "/tmp/alpha").await;
    let run_id = seed_run(&pool, "manual_all", "running").await;
    let queued = seed_run_project(&pool, run_id, project_id, "queued").await;

    let app = router_with_pool(&temp, pool.clone()).await;
    let (status, json) = post_cancel(app, run_id).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "cancelled");
    assert_eq!(rp_state(&pool, queued).await, "cancelled");
    assert!(
        rp_started_at(&pool, queued).await.is_none(),
        "a cancelled queued project must never have been claimed (started_at null)"
    );
}

#[tokio::test]
async fn cancel_api_unknown_run_returns_404() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let app = router_with_pool(&temp, pool).await;
    let (status, _json) = post_cancel(app, 9999).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_api_terminal_run_returns_409_without_modifying_rows() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    for terminal in ["success", "cancelled"] {
        let project_id = seed_project(&pool, terminal, &format!("/tmp/{terminal}")).await;
        let run_id = seed_run(&pool, "manual_all", terminal).await;
        // A project row left in a non-terminal state proves the 409 path touches
        // nothing: if the endpoint modified rows, this would flip.
        let rp = seed_run_project(&pool, run_id, project_id, "queued").await;

        let app = router_with_pool(&temp, pool.clone()).await;
        let (status, _json) = post_cancel(app, run_id).await;

        assert_eq!(status, StatusCode::CONFLICT, "terminal={terminal}");
        assert_eq!(
            rp_state(&pool, rp).await,
            "queued",
            "409 must not modify any run_projects row (terminal={terminal})"
        );
        assert_eq!(run_status(&pool, run_id).await, terminal);
    }
}

#[tokio::test]
async fn queued_projects_are_cancelled_and_never_claimed() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let project_id = seed_project(&pool, "alpha", "/tmp/alpha").await;
    let run_id = seed_run(&pool, "manual_all", "running").await;
    let running = seed_run_project(&pool, run_id, project_id, "running").await;
    let project2 = seed_project(&pool, "beta", "/tmp/beta").await;
    let queued = seed_run_project(&pool, run_id, project2, "queued").await;

    runs::cancel_run(&pool, run_id).await.expect("cancel run");

    assert_eq!(rp_state(&pool, queued).await, "cancelled");
    // The running row is left for the token-kill path; only queued rows are
    // flipped directly by the API.
    assert_eq!(rp_state(&pool, running).await, "running");
    assert!(
        fetch_next_queued_run_project(&pool)
            .await
            .expect("claim")
            .is_none(),
        "no queued project of a cancelled run may be claimed"
    );
}

// ---------------------------------------------------------------------------
// Task 3.2 — cancellation is scoped to one run
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancellation_is_scoped_to_one_run() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let pa = seed_project(&pool, "alpha", "/tmp/alpha").await;
    let run_a = seed_run(&pool, "manual_all", "running").await;
    let rp_a = seed_run_project(&pool, run_a, pa, "queued").await;

    let pb = seed_project(&pool, "beta", "/tmp/beta").await;
    let run_b = seed_run(&pool, "manual_all", "running").await;
    let rp_b = seed_run_project(&pool, run_b, pb, "queued").await;

    runs::cancel_run(&pool, run_a).await.expect("cancel run a");

    assert_eq!(run_status(&pool, run_a).await, "cancelled");
    assert_eq!(rp_state(&pool, rp_a).await, "cancelled");
    // Run B is entirely untouched and still claimable.
    assert_eq!(run_status(&pool, run_b).await, "running");
    assert_eq!(rp_state(&pool, rp_b).await, "queued");

    let claimed = fetch_next_queued_run_project(&pool)
        .await
        .expect("claim")
        .expect("run B's project remains claimable");
    assert_eq!(claimed.run_id, run_b);
}

// ---------------------------------------------------------------------------
// Task 4.1 — cancellation preserves and ingests produced outputs
// ---------------------------------------------------------------------------

/// Stand up an MR-scan run whose executor writes a draft then hangs, with the
/// triage author already bound to a person. Returns the pieces needed to cancel
/// mid-flight. Caller must hold `ENV_TEST_LOCK`.
async fn setup_mr_cancel(
    temp: &tempfile::TempDir,
) -> (sqlx::SqlitePool, i64, i64, i64, std::path::PathBuf) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml = temp.path().join("projects.yaml");
    std::fs::write(&yaml, "projects: []\n").expect("yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml);

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
    let project_id =
        seed_project(&pool, "alpha", &temp.path().join("repos/alpha").display().to_string()).await;

    // Bind the triage author so the observation-folder gate passes.
    sqlx::query("INSERT INTO people (display_name) VALUES ('Alice Chen')")
        .execute(&pool)
        .await
        .expect("insert person");
    sqlx::query(
        "INSERT INTO person_identities (person_id, kind, value)
         VALUES ((SELECT id FROM people WHERE display_name = 'Alice Chen'), 'git_email', 'alice@example.com')",
    )
    .execute(&pool)
    .await
    .expect("bind email");

    let run_id = runs::create_manual_mr_scan_run(&pool, project_id, false)
        .await
        .expect("create mr scan run");
    let draft_path =
        runs::mr_poll_draft_dir(temp.path(), run_id, project_id).join("mr-68-round-1.md");
    std::env::set_var("REVIEWER_TEST_DRAFT_FILE", &draft_path);

    let rp_id: i64 =
        sqlx::query_scalar("SELECT id FROM run_projects WHERE run_id = ? AND project_id = ?")
            .bind(run_id)
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .expect("run project id");

    (pool, run_id, rp_id, project_id, draft_path)
}

fn mr_job(rp_id: i64, run_id: i64, project_id: i64, temp: &tempfile::TempDir) -> RunProjectRow {
    RunProjectRow { id: rp_id,
    run_id,
    project_id,
    name: "alpha".into(),
    repo_path: temp.path().join("repos/alpha").display().to_string(),
    trigger: "manual_mr_poll".into(),
    mr_scan_force: 0, person_id: None }
}

fn clear_mr_env() {
    for key in [
        "REVIEWER_EXECUTOR",
        "REVIEWER_TRIAGE_SCRIPT",
        "REVIEWER_TEST_DRAFT_FILE",
        "REVIEWER_TEST_MR_IID",
    ] {
        std::env::remove_var(key);
    }
}

#[tokio::test]
async fn cancellation_preserves_and_ingests_draft() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let (pool, run_id, rp_id, project_id, draft_path) = setup_mr_cancel(&temp).await;

    let config = test_config();
    let job = mr_job(rp_id, run_id, project_id, &temp);
    let shutdown = CancellationToken::new();
    let run_token = shutdown.child_token();

    let fire = run_token.clone();
    tokio::spawn(async move {
        // Draft is written immediately; cancel while the executor still hangs.
        tokio::time::sleep(Duration::from_millis(800)).await;
        fire.cancel();
    });

    process_run_project(&pool, &config, job, 30, run_token, shutdown)
        .await
        .expect("process mr run project");

    assert_eq!(rp_state(&pool, rp_id).await, "cancelled");
    assert!(draft_path.is_file(), "draft written before cancel must survive");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mr_reviews WHERE project_id = ? AND mr_iid = 68 AND status = 'draft'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .expect("count drafts");
    assert_eq!(count, 1, "the on-disk draft must be ingested despite cancellation");

    clear_mr_env();
}

#[tokio::test]
async fn ingest_failure_does_not_block_cancellation() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let (pool, run_id, rp_id, project_id, _draft_path) = setup_mr_cancel(&temp).await;

    // Make only the draft INSERT fail, leaving the inbox-gate SELECTs working,
    // so ingest errors exactly where the cancellation path must not be blocked.
    sqlx::query(
        "CREATE TRIGGER block_mr_insert BEFORE INSERT ON mr_reviews
         BEGIN SELECT RAISE(FAIL, 'ingest blocked'); END",
    )
    .execute(&pool)
    .await
    .expect("create trigger");

    let config = test_config();
    let job = mr_job(rp_id, run_id, project_id, &temp);
    let shutdown = CancellationToken::new();
    let run_token = shutdown.child_token();

    let fire = run_token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        fire.cancel();
    });

    process_run_project(&pool, &config, job, 30, run_token, shutdown)
        .await
        .expect("process mr run project");

    assert_eq!(
        rp_state(&pool, rp_id).await,
        "cancelled",
        "a failed ingest must not stop the run from reaching cancelled"
    );

    clear_mr_env();
}

// ---------------------------------------------------------------------------
// Task 4.2 — startup recovery leaves cancelled rows untouched
// ---------------------------------------------------------------------------

#[tokio::test]
async fn startup_recovery_leaves_cancelled_rows_untouched() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let project_id = seed_project(&pool, "alpha", "/tmp/alpha").await;
    let run_id = seed_run(&pool, "manual_all", "cancelled").await;
    let rp = seed_run_project(&pool, run_id, project_id, "cancelled").await;
    sqlx::query("UPDATE run_projects SET error = 'user cancelled' WHERE id = ?")
        .bind(rp)
        .execute(&pool)
        .await
        .expect("set error");

    runs::recover_orphaned_running_projects(&pool)
        .await
        .expect("recover");

    assert_eq!(rp_state(&pool, rp).await, "cancelled");
    let error: Option<String> = sqlx::query_scalar("SELECT error FROM run_projects WHERE id = ?")
        .bind(rp)
        .fetch_one(&pool)
        .await
        .expect("error");
    assert_eq!(error.as_deref(), Some("user cancelled"));
}
