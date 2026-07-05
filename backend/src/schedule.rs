use std::sync::Arc;

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
    pub per_project_timeout_sec: i64,
    pub max_concurrency: i64,
}

pub async fn load_schedule_config(pool: &SqlitePool) -> Result<ScheduleConfigRow, Error> {
    sqlx::query_as::<_, ScheduleConfigRow>(
        "SELECT enabled, cadence, weekday, run_time, per_project_timeout_sec, max_concurrency
         FROM schedule_config WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(Error::Database)
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
    let scheduler = JobScheduler::new().await.map_err(|err| {
        Error::SummaryParse(format!("scheduler init: {err}"))
    })?;

    let job_pool = pool.clone();
    let job_worker = worker.clone();
    scheduler
        .add(Job::new_async(cron.as_str(), move |_uuid, _lock| {
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

    info!("schedule cron registered: {cron}");
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

    #[test]
    fn build_weekly_cron_from_defaults() {
        let config = ScheduleConfigRow {
            enabled: 1,
            cadence: "weekly".into(),
            weekday: Some(0),
            run_time: "09:00".into(),
            per_project_timeout_sec: 600,
            max_concurrency: 2,
        };
        assert_eq!(build_cron_expression(&config).expect("cron"), "0 0 9 * * 1");
    }
}
