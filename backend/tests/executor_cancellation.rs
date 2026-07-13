use std::sync::Mutex;
use std::time::Duration;

use reviewer_server::db::init_pool;
use reviewer_server::executor::{
    execute_agent_turn, execute_mr_review, execute_weekly_batch, ExecuteOutcome,
};
use reviewer_server::runs::{self, ProjectRow};
use tokio_util::sync::CancellationToken;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

fn slow_executor_path() -> std::path::PathBuf {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if cfg!(windows) {
        fixtures.join("slow_executor.cmd")
    } else {
        fixtures.join("slow_executor.sh")
    }
}

/// Weekly batch executor must race the child wait against cancellation: when
/// the shutdown token fires first, the child process tree is killed and the
/// outcome is Failed (not SkippedTimeout), with an error identifying
/// shutdown interruption.
#[tokio::test]
async fn execute_weekly_batch_fails_on_cancel_not_timeout() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    std::env::set_var("REVIEWER_EXECUTOR", slow_executor_path());

    let pool = init_pool(temp.path()).await.expect("init pool");
    std::fs::create_dir_all(temp.path().join("repos/alpha")).expect("repo dir");

    sqlx::query("INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)")
        .bind(temp.path().join("repos/alpha").display().to_string())
        .execute(&pool)
        .await
        .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let project = ProjectRow {
        id: 1,
        name: "alpha".into(),
        repo_path: temp.path().join("repos/alpha").display().to_string(),
    };
    let working_dir = temp.path().join("repos/alpha");

    let token = CancellationToken::new();
    let cancel_token = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel_token.cancel();
    });

    let (outcome, _duration_sec, error) = execute_weekly_batch(
        &pool,
        &config,
        run_id,
        &project,
        &working_dir,
        30,
        token,
    )
    .await
    .expect("execute weekly batch");

    assert_eq!(
        outcome,
        ExecuteOutcome::Failed,
        "cancellation must not be reported as SkippedTimeout"
    );
    let error = error.expect("error message present");
    assert!(
        error.contains(runs::SHUTDOWN_INTERRUPTED_ERROR),
        "error={error}"
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
}

/// MR review executor must exhibit the same cancel-wins-over-wait behavior
/// as the weekly batch executor.
#[tokio::test]
async fn execute_mr_review_fails_on_cancel_not_timeout() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    std::env::set_var("REVIEWER_EXECUTOR", slow_executor_path());

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let working_dir = temp.path().to_path_buf();
    let manifest_path = temp.path().join("manifest.json");
    std::fs::write(&manifest_path, "{}").expect("write manifest");

    let token = CancellationToken::new();
    let cancel_token = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel_token.cancel();
    });

    let result = execute_mr_review(
        &config,
        &working_dir,
        &manifest_path,
        30,
        config.reviewer_agent(),
        token,
    )
    .await;

    assert_eq!(
        result.outcome,
        ExecuteOutcome::Failed,
        "cancellation must not be reported as SkippedTimeout"
    );
    let error = result.error.expect("error message present");
    assert!(
        error.contains(runs::SHUTDOWN_INTERRUPTED_ERROR),
        "error={error}"
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
}

/// HTTP `agent-turn` uses the same cancellation token from application
/// state; when process shutdown cancels it, the child process tree is
/// killed and the turn fails identifying shutdown (not an ordinary agent
/// failure), so an in-flight clarification never leaks a reviewer process.
#[tokio::test]
async fn execute_agent_turn_fails_on_cancel_with_shutdown_error() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    std::env::set_var("REVIEWER_EXECUTOR", slow_executor_path());

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let working_dir = temp.path().to_path_buf();

    let token = CancellationToken::new();
    let cancel_token = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel_token.cancel();
    });

    let result = execute_agent_turn(
        &config,
        &working_dir,
        "sess-1",
        "clarify please",
        config.reviewer_agent(),
        token,
    )
    .await;

    let err = result.expect_err("cancellation must surface as an error, not a reply");
    assert!(
        err.to_string().contains(runs::SHUTDOWN_INTERRUPTED_ERROR),
        "error={err}"
    );

    std::env::remove_var("REVIEWER_EXECUTOR");
}

/// When the child exits before cancellation fires, normal success/timeout
/// semantics still apply (cancel and timeout must not be confused).
#[tokio::test]
async fn execute_weekly_batch_still_reports_timeout_when_cancel_not_fired() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    std::env::set_var("REVIEWER_EXECUTOR", slow_executor_path());

    let pool = init_pool(temp.path()).await.expect("init pool");
    std::fs::create_dir_all(temp.path().join("repos/alpha")).expect("repo dir");

    sqlx::query("INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)")
        .bind(temp.path().join("repos/alpha").display().to_string())
        .execute(&pool)
        .await
        .expect("insert project");

    let run_id = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('manual_all', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run")
    .last_insert_rowid();

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let project = ProjectRow {
        id: 1,
        name: "alpha".into(),
        repo_path: temp.path().join("repos/alpha").display().to_string(),
    };
    let working_dir = temp.path().join("repos/alpha");

    // Token never cancelled: the short timeout must still win as before.
    let token = CancellationToken::new();

    let (outcome, _duration_sec, error) =
        execute_weekly_batch(&pool, &config, run_id, &project, &working_dir, 1, token)
            .await
            .expect("execute weekly batch");

    assert_eq!(outcome, ExecuteOutcome::SkippedTimeout);
    assert!(error.is_none());

    std::env::remove_var("REVIEWER_EXECUTOR");
}
