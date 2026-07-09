## ADDED Requirements

### Requirement: MR poll cron triggers scheduled scans on an independent interval

The backend SHALL register a second cron job at startup (independent of the weekly batch cron) that fires every `schedule_config.mr_poll_interval_min` minutes. On each firing, the job MUST create a `runs` row with `trigger='mr_poll'`, insert one `run_projects` row per project with `is_git_repo=1` and `state='queued'`, and enqueue work for the worker pool, applying the same per-project deduplication lock used by the weekly batch (a project already `queued` or `running` under any active run MUST NOT be enqueued again).

If `schedule_config.enabled` is `0`, the MR poll cron MUST still be registered and fire independently of the weekly batch enabled flag, because the two tracks have separate operational cadences.

#### Scenario: MR poll fires on its configured interval

- **WHEN** `mr_poll_interval_min` is `60` and one hour elapses with the scheduler running
- **THEN** a new `runs` row with `trigger='mr_poll'` is created covering all healthy projects

#### Scenario: MR poll skips a project already locked by the weekly track

- **WHEN** the MR poll cron fires while a project has an active `run_projects` row with `state='running'` from a `trigger='schedule'` run
- **THEN** that project is not enqueued into the new `mr_poll` run, and other healthy projects are still enqueued
