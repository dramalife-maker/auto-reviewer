use std::sync::Arc;

use chrono::{Datelike, Duration, FixedOffset, NaiveDateTime, NaiveTime, Utc, Weekday};
use sqlx::SqlitePool;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

use crate::runs::create_scheduled_run;
use crate::worker::RunWorker;
use crate::Error;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduleConfigRow {
    pub enabled: i64,
    pub cadence: String,
    pub weekday: Option<i64>,
    pub run_time: String,
    pub tz_offset_min: i64,
    pub per_project_timeout_sec: i64,
    pub max_concurrency: i64,
}

pub async fn load_schedule_config(pool: &SqlitePool) -> Result<ScheduleConfigRow, Error> {
    sqlx::query_as::<_, ScheduleConfigRow>(
        "SELECT enabled, cadence, weekday, run_time, tz_offset_min,
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

pub async fn start_scheduler(pool: SqlitePool, worker: Arc<RunWorker>) -> Result<(), Error> {
    let config = load_schedule_config(&pool).await?;
    if config.enabled == 0 {
        info!("schedule disabled; cron not registered");
        return Ok(());
    }

    let cron = build_cron_expression(&config)?;
    let tz = schedule_timezone(&config)?;
    let scheduler = JobScheduler::new().await.map_err(|err| {
        Error::SummaryParse(format!("scheduler init: {err}"))
    })?;

    let job_pool = pool.clone();
    let job_worker = worker.clone();
    scheduler
        .add(Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let pool = job_pool.clone();
            let worker = job_worker.clone();
            Box::pin(async move {
                match trigger_scheduled_run(&pool).await {
                    Ok(Some(_run_id)) => worker.wake(),
                    Ok(None) => {}
                    Err(err) => error!("scheduled run failed: {err}"),
                }
            })
        }).map_err(|err| Error::SummaryParse(format!("scheduler job: {err}")))?)
        .await
        .map_err(|err| Error::SummaryParse(format!("scheduler add job: {err}")))?;

    scheduler
        .start()
        .await
        .map_err(|err| Error::SummaryParse(format!("scheduler start: {err}")))?;

    info!("schedule cron registered: {cron} (UTC offset {} min)", config.tz_offset_min);
    Ok(())
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
    fn default_timezone_is_taipei() {
        let tz = schedule_timezone(&sample_config()).expect("tz");
        assert_eq!(tz, FixedOffset::east_opt(8 * 3600).unwrap());
    }

    #[test]
    fn invalid_timezone_offset_is_rejected() {
        let mut config = sample_config();
        config.tz_offset_min = 100_000; // out of ±24h range
        assert!(schedule_timezone(&config).is_err());
    }
}
