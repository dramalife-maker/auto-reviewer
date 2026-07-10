## ADDED Requirements

### Requirement: Schedule configuration can be updated via API

The backend SHALL expose `PATCH /api/schedule` accepting a JSON object with any subset of:

- `enabled` (boolean)
- `weekday` (integer 0–6, where 0 is Monday)
- `run_time` (string `HH:MM`)
- `tz_offset_min` (integer; MUST form a valid fixed UTC offset)
- `per_project_timeout_sec` (integer ≥ 1)
- `max_concurrency` (integer ≥ 1)
- `mr_poll_interval_min` (integer; existing validation rules MUST apply, including disable when ≤ 0 and multiples of 60 when ≥ 60)
- `cadence` (optional string; if present MUST be `weekly`)

On success the backend MUST persist the provided fields on `schedule_config` id=1 and return HTTP 200 with the full schedule configuration response (including labels and `next_weekly_run_at`).

Invalid values MUST return HTTP 400 and MUST NOT persist partial updates for that request.

Omitting a field MUST leave that column unchanged.

#### Scenario: Update weekly run time and weekday

- **WHEN** a client patches `{ "weekday": 2, "run_time": "10:30" }`
- **THEN** `schedule_config` stores weekday 2 and run_time `10:30`
- **AND** the response `weekly_label` reflects 週三 10:30

#### Scenario: Reject non-weekly cadence

- **WHEN** a client patches `{ "cadence": "daily" }`
- **THEN** the response status is 400
- **AND** `schedule_config.cadence` remains unchanged

#### Scenario: Reject invalid timeout

- **WHEN** a client patches `{ "per_project_timeout_sec": 0 }`
- **THEN** the response status is 400

### Requirement: Dashboard schedule panel edits schedule settings

The dashboard schedule panel SHALL allow editing the fields supported by `PATCH /api/schedule` (except `cadence`, which MUST be shown as read-only weekly).

After a successful save, the UI MUST inform the operator that changes affecting cron registration require restarting `reviewer-server`, while `per_project_timeout_sec` and `max_concurrency` apply to the next run without restart.

#### Scenario: Save schedule from dashboard

- **WHEN** a manager updates weekday and MR poll interval on the dashboard and saves
- **THEN** the client calls `PATCH /api/schedule` with those fields
- **AND** on success the panel shows the updated labels and a restart notice for cron-related fields

### Requirement: Missed weekly schedule is detected for catch-up

When `schedule_config.enabled=1`, the backend MUST compute the most recent weekly due timestamp `due_at` that is strictly before now, using `weekday`, `run_time`, and `tz_offset_min`.

The due window is covered when at least one `runs` row exists with:

- `trigger` in (`schedule`, `manual_all`)
- `started_at` greater than or equal to `due_at` minus 6 hours
- `status` in (`success`, `partial`, `running`, `queued`)

If the window is not covered, schedule/dashboard responses MUST include `missed_weekly_run` as an object `{ "due_at": "<ISO-8601>", "label": "<human-readable>" }`. Otherwise `missed_weekly_run` MUST be null.

When `enabled=0`, `missed_weekly_run` MUST be null.

The detector MUST evaluate only the single most recent due window, not older weeks.

MR poll gaps MUST NOT produce a missed-run signal.

#### Scenario: Missed run reported after downtime

- **GIVEN** enabled weekly schedule with due_at in the past
- **AND** no covering `schedule` or `manual_all` run near that due_at
- **WHEN** a client fetches the dashboard or schedule config
- **THEN** `missed_weekly_run` is non-null and its `due_at` matches that window

#### Scenario: Covered window suppresses missed signal

- **GIVEN** a `manual_all` run started within 6 hours after due_at with status `success`
- **WHEN** a client fetches the schedule config
- **THEN** `missed_weekly_run` is null

#### Scenario: Disabled schedule never reports missed run

- **GIVEN** `schedule_config.enabled=0`
- **AND** the last weekly due_at has no covering run
- **WHEN** a client fetches the schedule config
- **THEN** `missed_weekly_run` is null

##### Example: coverage check

| due_at (local) | covering run | missed_weekly_run |
| --- | --- | --- |
| Mon 09:00, now Tue | none | non-null |
| Mon 09:00, now Tue | `manual_all` success started Mon 09:15 | null |
| Mon 09:00, now Tue | `manual_project` only | non-null |
| enabled=0 | none | null |

### Requirement: Operator can confirm weekly catch-up run

The backend SHALL expose `POST /api/schedule/catch-up` that enqueues the same all-projects weekly batch pipeline as `manual_all` (creating a `runs` row the worker can execute).

On success the response MUST identify the created `run_id` (HTTP 202 or the project's existing create-run success shape).

If a conflicting in-flight run prevents enqueue, the response MUST be HTTP 409.

The dashboard SHALL show a banner when `missed_weekly_run` is non-null, with actions to confirm catch-up or dismiss for the browser session only (`sessionStorage`). Dismiss MUST NOT persist in the database; a later reload MUST show the banner again if the window is still missed.

#### Scenario: Catch-up creates a batch run

- **WHEN** a client posts `POST /api/schedule/catch-up` while no lock conflict exists
- **THEN** a new batch run is created and its `run_id` is returned

#### Scenario: Catch-up conflict returns 409

- **WHEN** a conflicting run already locks projects
- **AND** a client posts `POST /api/schedule/catch-up`
- **THEN** the response status is 409

#### Scenario: Dashboard banner offers catch-up

- **GIVEN** dashboard payload includes non-null `missed_weekly_run`
- **WHEN** the manager opens the dashboard
- **THEN** a banner offers immediate catch-up and a session-only dismiss action


