## ADDED Requirements

### Requirement: Schedule configuration is stored as a single row

The database SHALL contain table `schedule_config` with exactly one row (`id=1`) holding fields `enabled`, `cadence`, `weekday`, `run_time`, `per_project_timeout_sec`, and `max_concurrency` as defined in `docs/idea/schema.md`.

On first startup after migration, the server MUST seed defaults: `enabled=1`, `cadence='weekly'`, `weekday=0`, `run_time='09:00'`, `per_project_timeout_sec=600`, `max_concurrency=2`.

#### Scenario: Fresh database receives default schedule

- **WHEN** migrations run on an empty database
- **THEN** `schedule_config` contains one row with `run_time='09:00'` and `max_concurrency=2`

### Requirement: Enabled schedule triggers weekly batch runs

When `schedule_config.enabled=1`, the backend SHALL register a cron job matching `cadence`, `weekday`, and `run_time` that starts the same batch pipeline as `manual_all` with `runs.trigger='schedule'`.

When `enabled=0`, the cron job MUST NOT enqueue runs.

#### Scenario: Scheduled trigger creates run record

- **WHEN** the cron fires while `enabled=1` and no duplicate project lock exists
- **THEN** a new `runs` row exists with `trigger='schedule'`

#### Scenario: Disabled schedule does not enqueue

- **WHEN** `enabled=0` and the cron tick occurs
- **THEN** no new `runs` row is created

