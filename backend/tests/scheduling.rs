use reviewer_server::db::init_pool;
use reviewer_server::runs::{create_mr_poll_run, create_scheduled_run};
use reviewer_server::schedule::{
    load_schedule_config, trigger_mr_poll_run_unless_cancelled, trigger_scheduled_run,
    trigger_scheduled_run_unless_cancelled,
};
use reviewer_server::worker::RunWorker;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn schedule_config_seeded() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let config = load_schedule_config(&pool).await.expect("schedule config");
    assert_eq!(config.enabled, 1);
    assert_eq!(config.cadence, "weekly");
    assert_eq!(config.weekday, Some(0));
    assert_eq!(config.run_time, "09:00");
    assert_eq!(config.per_project_timeout_sec, 600);
    assert_eq!(config.max_concurrency, 2);
}

#[tokio::test]
async fn scheduled_run_creates_schedule_trigger() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 0)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let run_id = trigger_scheduled_run(&pool)
        .await
        .expect("trigger")
        .expect("run id");

    let trigger: String = sqlx::query_scalar("SELECT trigger FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("trigger");
    assert_eq!(trigger, "schedule");
}

#[tokio::test]
async fn mr_poll_skips_project_locked_by_weekly_track() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    for name in ["alpha", "beta"] {
        sqlx::query(
            "INSERT INTO projects (name, repo_path, is_git_repo) VALUES (?, ?, 1)",
        )
        .bind(name)
        .bind(temp.path().join(format!("repos/{name}")).display().to_string())
        .execute(&pool)
        .await
        .expect("insert project");
    }

    let alpha_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'alpha'")
        .fetch_one(&pool)
        .await
        .expect("alpha id");
    let beta_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE name = 'beta'")
        .fetch_one(&pool)
        .await
        .expect("beta id");

    let run_result = sqlx::query(
        "INSERT INTO runs (trigger, status, project_total) VALUES ('schedule', 'running', 1)",
    )
    .execute(&pool)
    .await
    .expect("insert run");
    let weekly_run_id = run_result.last_insert_rowid();

    sqlx::query(
        "INSERT INTO run_projects (run_id, project_id, state) VALUES (?, ?, 'running')",
    )
    .bind(weekly_run_id)
    .bind(alpha_id)
    .execute(&pool)
    .await
    .expect("lock alpha");

    let mr_run_id = create_mr_poll_run(&pool).await.expect("mr poll run");

    let enqueued: Vec<i64> = sqlx::query_scalar(
        "SELECT project_id FROM run_projects WHERE run_id = ? ORDER BY project_id",
    )
    .bind(mr_run_id)
    .fetch_all(&pool)
    .await
    .expect("enqueued");
    assert_eq!(enqueued, vec![beta_id]);
}

#[tokio::test]
async fn disabled_schedule_does_not_enqueue() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query("UPDATE schedule_config SET enabled = 0 WHERE id = 1")
        .execute(&pool)
        .await
        .expect("disable schedule");

    let run_id = trigger_scheduled_run(&pool).await.expect("trigger");
    assert!(run_id.is_none());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn create_scheduled_run_uses_schedule_trigger() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(temp.path()).await.expect("init pool");

    let run_id = create_scheduled_run(&pool).await.expect("create scheduled run");
    let trigger: String = sqlx::query_scalar("SELECT trigger FROM runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .expect("trigger");
    assert_eq!(trigger, "schedule");
}

/// The weekly cron job callback delegates to this wrapper; once the shared
/// shutdown token is cancelled, it must not enqueue a new "schedule" run
/// even though the cron config itself is enabled.
#[tokio::test]
async fn weekly_cron_callback_does_not_enqueue_after_cancel() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let pool = init_pool(temp.path()).await.expect("init pool");

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let cancel = CancellationToken::new();
    cancel.cancel();
    let worker = RunWorker::spawn(config, pool.clone(), cancel.clone());

    trigger_scheduled_run_unless_cancelled(&pool, &worker, &cancel).await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 0, "no run must be enqueued after cancellation");
}

/// Same guard as the weekly job, for the mr-poll cron callback.
#[tokio::test]
async fn mr_poll_cron_callback_does_not_enqueue_after_cancel() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let pool = init_pool(temp.path()).await.expect("init pool");

    sqlx::query(
        "INSERT INTO projects (name, repo_path, is_git_repo) VALUES ('alpha', ?, 1)",
    )
    .bind(temp.path().join("repos/alpha").display().to_string())
    .execute(&pool)
    .await
    .expect("insert project");

    let config = reviewer_server::config::AppConfig::from_env().expect("config");
    let cancel = CancellationToken::new();
    cancel.cancel();
    let worker = RunWorker::spawn(config, pool.clone(), cancel.clone());

    trigger_mr_poll_run_unless_cancelled(&pool, &worker, &cancel).await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 0, "no mr poll run must be enqueued after cancellation");
}
