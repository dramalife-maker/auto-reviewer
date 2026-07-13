use std::sync::Arc;

use chrono::{Datelike, Duration, FixedOffset, NaiveDateTime, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio_cron_scheduler::{Job, JobScheduler};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::runs::{create_mr_poll_run, create_scheduled_run};
use crate::worker::RunWorker;
use crate::Error;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduleConfigRow {
    pub enabled: i64,
    pub cadence: String,
    pub weekday: Option<i64>,
    pub run_time: String,
    pub tz_offset_min: i64,
    pub mr_poll_interval_min: i64,
    pub per_project_timeout_sec: i64,
    pub max_concurrency: i64,
}

pub async fn load_schedule_config(pool: &SqlitePool) -> Result<ScheduleConfigRow, Error> {
    sqlx::query_as::<_, ScheduleConfigRow>(
        "SELECT enabled, cadence, weekday, run_time, tz_offset_min, mr_poll_interval_min,
                per_project_timeout_sec, max_concurrency
         FROM schedule_config WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(Error::Database)
}

/// The timezone `run_time` is interpreted in (offset from UTC).
fn schedule_timezone(config: &ScheduleConfigRow) -> Result<FixedOffset, Error> {
    let secs = (config.tz_offset_min as i32) * 60;
    FixedOffset::east_opt(secs).ok_or_else(|| {
        Error::SummaryParse(format!("invalid tz_offset_min: {}", config.tz_offset_min))
    })
}

pub async fn trigger_scheduled_run(pool: &SqlitePool) -> Result<Option<i64>, Error> {
    let config = load_schedule_config(pool).await?;
    if config.enabled == 0 {
        return Ok(None);
    }

    match create_scheduled_run(pool).await {
        Ok(run_id) => Ok(Some(run_id)),
        Err(Error::RunConflict) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Cancellation-aware wrapper the weekly cron job callback delegates to.
/// Exists as a standalone function so the "no enqueue after cancel" guard
/// is unit-testable without waiting on real cron timing.
pub async fn trigger_scheduled_run_unless_cancelled(
    pool: &SqlitePool,
    worker: &RunWorker,
    cancel: &CancellationToken,
) {
    if cancel.is_cancelled() {
        return;
    }
    match trigger_scheduled_run(pool).await {
        Ok(Some(_run_id)) => worker.wake(),
        Ok(None) => {}
        Err(err) => error!("scheduled run failed: {err}"),
    }
}

pub async fn trigger_mr_poll_run(pool: &SqlitePool) -> Result<Option<i64>, Error> {
    let config = load_schedule_config(pool).await?;
    if config.mr_poll_interval_min <= 0 {
        return Ok(None);
    }

    match create_mr_poll_run(pool).await {
        Ok(run_id) => Ok(Some(run_id)),
        Err(Error::RunConflict) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Cancellation-aware wrapper the mr-poll cron job callback delegates to.
/// See [`trigger_scheduled_run_unless_cancelled`].
pub async fn trigger_mr_poll_run_unless_cancelled(
    pool: &SqlitePool,
    worker: &RunWorker,
    cancel: &CancellationToken,
) {
    if cancel.is_cancelled() {
        return;
    }
    match trigger_mr_poll_run(pool).await {
        Ok(Some(_run_id)) => worker.wake(),
        Ok(None) => {}
        Err(err) => error!("mr poll run failed: {err}"),
    }
}

pub async fn start_scheduler(
    pool: SqlitePool,
    worker: Arc<RunWorker>,
    cancel: CancellationToken,
) -> Result<(), Error> {
    let config = load_schedule_config(&pool).await?;
    let tz = schedule_timezone(&config)?;
    let mut scheduler = JobScheduler::new().await.map_err(|err| {
        Error::SummaryParse(format!("scheduler init: {err}"))
    })?;

    if config.enabled != 0 {
        let cron = build_cron_expression(&config)?;
        let job_pool = pool.clone();
        let job_worker = worker.clone();
        let job_cancel = cancel.clone();
        scheduler
            .add(Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
                let pool = job_pool.clone();
                let worker = job_worker.clone();
                let cancel = job_cancel.clone();
                Box::pin(async move {
                    trigger_scheduled_run_unless_cancelled(&pool, &worker, &cancel).await;
                })
            })
            .map_err(|err| Error::SummaryParse(format!("scheduler job: {err}")))?)
            .await
            .map_err(|err| Error::SummaryParse(format!("scheduler add job: {err}")))?;
        info!(
            "weekly schedule cron registered: {cron} (UTC offset {} min)",
            config.tz_offset_min
        );
    } else {
        info!("weekly schedule disabled; weekly cron not registered");
    }

    start_mr_poll_scheduler(&scheduler, &pool, worker, &config, tz, cancel.clone()).await?;

    scheduler
        .start()
        .await
        .map_err(|err| Error::SummaryParse(format!("scheduler start: {err}")))?;

    // Stop the cron scheduler from firing new jobs once shutdown begins.
    // `JobScheduler` is not `Send + Sync`-shared here, so the shutdown call
    // is driven from a dedicated task that owns it for its remaining life.
    tokio::spawn(async move {
        cancel.cancelled().await;
        if let Err(err) = scheduler.shutdown().await {
            error!("scheduler shutdown error: {err}");
        }
    });

    Ok(())
}

async fn start_mr_poll_scheduler(
    scheduler: &JobScheduler,
    pool: &SqlitePool,
    worker: Arc<RunWorker>,
    config: &ScheduleConfigRow,
    tz: FixedOffset,
    cancel: CancellationToken,
) -> Result<(), Error> {
    if config.mr_poll_interval_min <= 0 {
        info!("mr poll disabled (mr_poll_interval_min <= 0)");
        return Ok(());
    }

    let cron = build_mr_poll_cron_expression(config.mr_poll_interval_min)?;
    let job_pool = pool.clone();
    let job_worker = worker.clone();
    scheduler
        .add(Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let pool = job_pool.clone();
            let worker = job_worker.clone();
            let cancel = cancel.clone();
            Box::pin(async move {
                trigger_mr_poll_run_unless_cancelled(&pool, &worker, &cancel).await;
            })
        })
        .map_err(|err| Error::SummaryParse(format!("mr poll scheduler job: {err}")))?)
        .await
        .map_err(|err| Error::SummaryParse(format!("mr poll scheduler add job: {err}")))?;

    info!(
        "mr poll cron registered: {cron} every {} min (UTC offset {} min)",
        config.mr_poll_interval_min, config.tz_offset_min
    );
    Ok(())
}

fn build_mr_poll_cron_expression(interval_min: i64) -> Result<String, Error> {
    if interval_min <= 0 {
        return Err(Error::SummaryParse(
            "mr_poll_interval_min must be positive".into(),
        ));
    }
    if interval_min >= 60 {
        if interval_min % 60 != 0 {
            return Err(Error::SummaryParse(format!(
                "mr_poll_interval_min must divide 60 when >= 60: {interval_min}"
            )));
        }
        let hours = interval_min / 60;
        return Ok(format!("0 0 */{hours} * * *"));
    }
    Ok(format!("0 */{interval_min} * * * *"))
}

fn build_cron_expression(config: &ScheduleConfigRow) -> Result<String, Error> {
    if config.cadence != "weekly" {
        return Err(Error::SummaryParse(format!(
            "unsupported cadence: {}",
            config.cadence
        )));
    }

    let (hour, minute) = parse_run_time(&config.run_time)?;
    let weekday = config.weekday.unwrap_or(0);
    let cron_weekday = spec_weekday_to_cron(weekday);
    Ok(format!("0 {minute} {hour} * * {cron_weekday}"))
}

fn parse_run_time(run_time: &str) -> Result<(u32, u32), Error> {
    let mut parts = run_time.split(':');
    let hour: u32 = parts
        .next()
        .ok_or_else(|| Error::SummaryParse("invalid run_time".into()))?
        .parse()
        .map_err(|_| Error::SummaryParse(format!("invalid run_time hour: {run_time}")))?;
    let minute: u32 = parts
        .next()
        .ok_or_else(|| Error::SummaryParse("invalid run_time".into()))?
        .parse()
        .map_err(|_| Error::SummaryParse(format!("invalid run_time minute: {run_time}")))?;
    Ok((hour, minute))
}

pub fn format_schedule_label(config: &ScheduleConfigRow) -> String {
    let weekday_names = ["一", "二", "三", "四", "五", "六", "日"];
    let weekday = config.weekday.unwrap_or(0).clamp(0, 6) as usize;
    format!("每週{} {}", weekday_names[weekday], config.run_time)
}

pub fn format_mr_poll_label(interval_min: i64) -> String {
    if interval_min <= 0 {
        return "已停用".to_string();
    }
    if interval_min < 60 {
        return format!("每 {interval_min} 分鐘");
    }
    if interval_min % 60 == 0 {
        let hours = interval_min / 60;
        return format!("每 {hours} 小時");
    }
    format!("每 {interval_min} 分鐘")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MissedWeeklyRun {
    pub due_at: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleUpdateInput {
    pub enabled: Option<bool>,
    pub weekday: Option<i64>,
    pub run_time: Option<String>,
    pub tz_offset_min: Option<i64>,
    pub per_project_timeout_sec: Option<i64>,
    pub max_concurrency: Option<i64>,
    pub mr_poll_interval_min: Option<i64>,
    pub cadence: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScheduleConfigResponse {
    pub enabled: bool,
    pub cadence: String,
    pub weekday: Option<i64>,
    pub run_time: String,
    pub tz_offset_min: i64,
    pub mr_poll_interval_min: i64,
    pub per_project_timeout_sec: i64,
    pub max_concurrency: i64,
    pub weekly_label: String,
    pub mr_poll_label: String,
    pub next_weekly_run_at: Option<String>,
    pub missed_weekly_run: Option<MissedWeeklyRun>,
}

pub async fn get_schedule_config_response(
    pool: &SqlitePool,
) -> Result<ScheduleConfigResponse, Error> {
    let config = load_schedule_config(pool).await?;
    let missed_weekly_run = detect_missed_weekly_run(pool, &config).await?;
    Ok(ScheduleConfigResponse {
        enabled: config.enabled != 0,
        cadence: config.cadence.clone(),
        weekday: config.weekday,
        run_time: config.run_time.clone(),
        tz_offset_min: config.tz_offset_min,
        mr_poll_interval_min: config.mr_poll_interval_min,
        per_project_timeout_sec: config.per_project_timeout_sec,
        max_concurrency: config.max_concurrency,
        weekly_label: format_schedule_label(&config),
        mr_poll_label: format_mr_poll_label(config.mr_poll_interval_min),
        next_weekly_run_at: compute_next_run_at(&config)?,
        missed_weekly_run,
    })
}

fn validate_schedule_update(input: &ScheduleUpdateInput) -> Result<(), Error> {
    if let Some(cadence) = &input.cadence {
        if cadence != "weekly" {
            return Err(Error::InvalidScheduleConfig(format!(
                "cadence must be weekly, got {cadence}"
            )));
        }
    }
    if let Some(weekday) = input.weekday {
        if !(0..=6).contains(&weekday) {
            return Err(Error::InvalidScheduleConfig(format!(
                "weekday must be 0–6, got {weekday}"
            )));
        }
    }
    if let Some(run_time) = &input.run_time {
        let (hour, minute) = parse_run_time(run_time).map_err(|_| {
            Error::InvalidScheduleConfig(format!("invalid run_time: {run_time}"))
        })?;
        if hour > 23 || minute > 59 {
            return Err(Error::InvalidScheduleConfig(format!(
                "invalid run_time: {run_time}"
            )));
        }
    }
    if let Some(tz_offset_min) = input.tz_offset_min {
        let secs = (tz_offset_min as i32).checked_mul(60).ok_or_else(|| {
            Error::InvalidScheduleConfig(format!("invalid tz_offset_min: {tz_offset_min}"))
        })?;
        FixedOffset::east_opt(secs).ok_or_else(|| {
            Error::InvalidScheduleConfig(format!("invalid tz_offset_min: {tz_offset_min}"))
        })?;
    }
    if let Some(timeout) = input.per_project_timeout_sec {
        if timeout < 1 {
            return Err(Error::InvalidScheduleConfig(
                "per_project_timeout_sec must be >= 1".into(),
            ));
        }
    }
    if let Some(concurrency) = input.max_concurrency {
        if concurrency < 1 {
            return Err(Error::InvalidScheduleConfig(
                "max_concurrency must be >= 1".into(),
            ));
        }
    }
    if let Some(interval) = input.mr_poll_interval_min {
        validate_mr_poll_interval(interval)?;
    }
    Ok(())
}

fn validate_mr_poll_interval(interval: i64) -> Result<(), Error> {
    if interval <= 0 {
        return Ok(());
    }
    build_mr_poll_cron_expression(interval).map_err(|err| match err {
        Error::SummaryParse(msg) => Error::InvalidScheduleConfig(msg),
        other => other,
    })?;
    Ok(())
}

pub async fn update_schedule_config(
    pool: &SqlitePool,
    input: ScheduleUpdateInput,
) -> Result<ScheduleConfigResponse, Error> {
    validate_schedule_update(&input)?;

    let current = load_schedule_config(pool).await?;
    let enabled = input
        .enabled
        .map(|v| if v { 1 } else { 0 })
        .unwrap_or(current.enabled);
    let weekday = input.weekday.or(current.weekday);
    let run_time = input
        .run_time
        .clone()
        .unwrap_or_else(|| current.run_time.clone());
    let tz_offset_min = input.tz_offset_min.unwrap_or(current.tz_offset_min);
    let per_project_timeout_sec = input
        .per_project_timeout_sec
        .unwrap_or(current.per_project_timeout_sec);
    let max_concurrency = input
        .max_concurrency
        .unwrap_or(current.max_concurrency);
    let mr_poll_interval_min = input
        .mr_poll_interval_min
        .unwrap_or(current.mr_poll_interval_min);
    let cadence = input
        .cadence
        .clone()
        .unwrap_or_else(|| current.cadence.clone());

    sqlx::query(
        "UPDATE schedule_config SET
            enabled = ?,
            cadence = ?,
            weekday = ?,
            run_time = ?,
            tz_offset_min = ?,
            per_project_timeout_sec = ?,
            max_concurrency = ?,
            mr_poll_interval_min = ?,
            updated_at = datetime('now')
         WHERE id = 1",
    )
    .bind(enabled)
    .bind(cadence)
    .bind(weekday)
    .bind(run_time)
    .bind(tz_offset_min)
    .bind(per_project_timeout_sec)
    .bind(max_concurrency)
    .bind(mr_poll_interval_min)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    get_schedule_config_response(pool).await
}

const MISSED_RUN_TOLERANCE_HOURS: i64 = 6;

pub fn compute_last_due_at(
    config: &ScheduleConfigRow,
    now: chrono::DateTime<FixedOffset>,
) -> Result<Option<chrono::DateTime<FixedOffset>>, Error> {
    let (hour, minute) = parse_run_time(&config.run_time)?;
    let target_weekday = spec_weekday_to_chrono_weekday(config.weekday.unwrap_or(0));
    let run_time = NaiveTime::from_hms_opt(hour, minute, 0).ok_or_else(|| {
        Error::SummaryParse(format!("invalid run_time: {}", config.run_time))
    })?;

    for offset in 0..8 {
        let candidate_date = now.date_naive() - Duration::days(offset);
        if candidate_date.weekday() != target_weekday {
            continue;
        }

        let candidate_dt = NaiveDateTime::new(candidate_date, run_time);
        let candidate = candidate_dt
            .and_local_timezone(now.timezone())
            .single()
            .ok_or_else(|| Error::SummaryParse("ambiguous local time".into()))?;
        if candidate < now {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn format_missed_due_label(due_at: chrono::DateTime<FixedOffset>) -> String {
    let weekday_names = ["一", "二", "三", "四", "五", "六", "日"];
    let weekday = due_at.weekday().num_days_from_monday() as usize;
    format!(
        "週{} {} {}",
        weekday_names[weekday],
        due_at.format("%m-%d"),
        due_at.format("%H:%M")
    )
}

pub async fn detect_missed_weekly_run(
    pool: &SqlitePool,
    config: &ScheduleConfigRow,
) -> Result<Option<MissedWeeklyRun>, Error> {
    if config.enabled == 0 {
        return Ok(None);
    }

    let tz = schedule_timezone(config)?;
    let now = Utc::now().with_timezone(&tz);
    let Some(due_at) = compute_last_due_at(config, now)? else {
        return Ok(None);
    };

    let window_start = due_at - Duration::hours(MISSED_RUN_TOLERANCE_HOURS);
    let window_start_utc = window_start
        .with_timezone(&Utc)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let covered: i64 = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM runs
            WHERE trigger IN ('schedule', 'manual_all')
              AND status IN ('success', 'partial', 'running', 'queued')
              AND started_at >= ?
         )",
    )
    .bind(&window_start_utc)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;

    if covered != 0 {
        return Ok(None);
    }

    Ok(Some(MissedWeeklyRun {
        due_at: due_at.to_rfc3339(),
        label: format_missed_due_label(due_at),
    }))
}

pub fn compute_next_run_at(config: &ScheduleConfigRow) -> Result<Option<String>, Error> {
    if config.enabled == 0 {
        return Ok(None);
    }

    let tz = schedule_timezone(config)?;
    let now = Utc::now().with_timezone(&tz);
    let (hour, minute) = parse_run_time(&config.run_time)?;
    let target_weekday = spec_weekday_to_chrono_weekday(config.weekday.unwrap_or(0));
    let run_time = NaiveTime::from_hms_opt(hour, minute, 0).ok_or_else(|| {
        Error::SummaryParse(format!("invalid run_time: {}", config.run_time))
    })?;

    for offset in 0..8 {
        let candidate_date = now.date_naive() + Duration::days(offset);
        if candidate_date.weekday() != target_weekday {
            continue;
        }

        let candidate_dt = NaiveDateTime::new(candidate_date, run_time);
        let candidate = candidate_dt
            .and_local_timezone(tz)
            .single()
            .ok_or_else(|| Error::SummaryParse("ambiguous local time".into()))?;
        if candidate > now {
            return Ok(Some(candidate.format("%m-%d %H:%M").to_string()));
        }
    }

    Ok(None)
}

fn spec_weekday_to_chrono_weekday(weekday: i64) -> Weekday {
    match weekday {
        0 => Weekday::Mon,
        1 => Weekday::Tue,
        2 => Weekday::Wed,
        3 => Weekday::Thu,
        4 => Weekday::Fri,
        5 => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

/// Spec weekday: 0=Monday … 6=Sunday. Cron (Sun=0): Mon=1 … Sat=6, Sun=0.
fn spec_weekday_to_cron(weekday: i64) -> u32 {
    match weekday {
        0 => 1,
        1 => 2,
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 6,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> ScheduleConfigRow {
        ScheduleConfigRow {
            enabled: 1,
            cadence: "weekly".into(),
            weekday: Some(0),
            run_time: "09:00".into(),
            tz_offset_min: 480,
            mr_poll_interval_min: 60,
            per_project_timeout_sec: 600,
            max_concurrency: 2,
        }
    }

    #[test]
    fn format_schedule_label_uses_weekday_and_time() {
        assert_eq!(
            format_schedule_label(&sample_config()),
            "每週一 09:00"
        );
    }

    #[test]
    fn compute_next_run_at_when_enabled() {
        let next = compute_next_run_at(&sample_config()).expect("next run");
        assert!(next.is_some());
    }

    #[test]
    fn build_weekly_cron_from_defaults() {
        assert_eq!(
            build_cron_expression(&sample_config()).expect("cron"),
            "0 0 9 * * 1"
        );
    }

    #[test]
    fn build_mr_poll_cron_every_minute() {
        assert_eq!(
            build_mr_poll_cron_expression(1).expect("cron"),
            "0 */1 * * * *"
        );
    }

    #[test]
    fn build_mr_poll_cron_hourly() {
        assert_eq!(
            build_mr_poll_cron_expression(60).expect("cron"),
            "0 0 */1 * * *"
        );
    }

    #[test]
    fn default_timezone_is_taipei() {
        let tz = schedule_timezone(&sample_config()).expect("tz");
        assert_eq!(tz, FixedOffset::east_opt(8 * 3600).unwrap());
    }

    #[test]
    fn invalid_timezone_offset_is_rejected() {
        let mut config = sample_config();
        config.tz_offset_min = 100_000;
        assert!(schedule_timezone(&config).is_err());
    }

    #[test]
    fn last_due_at_is_previous_weekday_occurrence() {
        let config = sample_config(); // Mon 09:00 UTC+8
        let tz = schedule_timezone(&config).expect("tz");
        // Tuesday 2026-07-07 10:00 Taipei → last due Mon 2026-07-06 09:00
        let now = NaiveDateTime::parse_from_str("2026-07-07 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_local_timezone(tz)
            .single()
            .unwrap();
        let due = compute_last_due_at(&config, now).expect("due").expect("some");
        assert_eq!(due.format("%Y-%m-%d %H:%M").to_string(), "2026-07-06 09:00");
    }

    #[test]
    fn missed_label_includes_weekday_and_time() {
        let tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let due = NaiveDateTime::parse_from_str("2026-07-06 09:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_local_timezone(tz)
            .single()
            .unwrap();
        assert_eq!(format_missed_due_label(due), "週一 07-06 09:00");
    }
}
