# scheduling Specification

## Purpose

TBD - created by archiving change 'cloud-reviewer-mvp'. Update Purpose after archive.

## Requirements

### Requirement: Schedule configuration is stored as a single row

The database SHALL contain table `schedule_config` with exactly one row (`id=1`) holding fields `enabled`, `cadence`, `weekday`, `run_time`, `tz_offset_min`, `per_project_timeout_sec`, and `max_concurrency` as defined in `docs/idea/schema.md`.

On first startup after migration, the server MUST seed defaults: `enabled=1`, `cadence='weekly'`, `weekday=0`, `run_time='09:00'`, `tz_offset_min=480`, `per_project_timeout_sec=600`, `max_concurrency=2`.

The `tz_offset_min` field is the timezone offset from UTC in minutes; the default `480` corresponds to Asia/Taipei (UTC+8).

#### Scenario: Fresh database receives default schedule

- **WHEN** migrations run on an empty database
- **THEN** `schedule_config` contains one row with `run_time='09:00'`, `tz_offset_min=480`, and `max_concurrency=2`


<!-- @trace
source: cloud-reviewer-mvp
updated: 2026-07-05
code:
  - README.md
  - backend/src/main.rs
  - frontend/src/main.ts
  - backend/src/server.rs
  - backend/src/runs.rs
  - crates/app-env/Cargo.toml
  - backend/src/projects.rs
  - frontend/src/types.ts
  - frontend/src/app.ts
  - backend/Cargo.toml
  - docs/idea/schema.md
  - Cargo.toml
  - frontend/src/api.ts
  - backend/migrations/001_initial.sql
  - .env.example
  - frontend/index.html
  - backend/src/state.rs
  - frontend/public/favicon.svg
  - crates/app-env/src/lib.rs
  - docs/idea/spec.md
  - backend/src/reports.rs
  - backend/src/schedule.rs
  - frontend/src/assets/typescript.svg
  - frontend/src/assets/hero.png
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/error.rs
  - backend/src/lib.rs
  - frontend/src/assets/vite.svg
  - frontend/src/style.css
  - frontend/vite.config.ts
  - backend/src/executor.rs
  - frontend/package.json
  - backend/src/summary.rs
  - projects.yaml
  - frontend/public/icons.svg
  - backend/src/db.rs
  - backend/src/worker.rs
  - frontend/tsconfig.json
  - skills/reviewer-batch/output-contract.md
  - backend/src/config.rs
tests:
  - backend/tests/fixtures/slow_executor.cmd
  - backend/tests/runs_execution.rs
  - backend/tests/scheduling.rs
  - backend/tests/project_config.rs
  - backend/tests/foundation.rs
  - backend/tests/report_reader.rs
-->

---
### Requirement: Enabled schedule triggers weekly batch runs

When `schedule_config.enabled=1`, the backend SHALL register a cron job matching `cadence`, `weekday`, and `run_time` that starts the same batch pipeline as `manual_all` with `runs.trigger='schedule'`. The backend MUST interpret `run_time` in the timezone given by `tz_offset_min` (offset from UTC in minutes), not in UTC.

When `enabled=0`, the cron job MUST NOT enqueue runs.

#### Scenario: Scheduled trigger creates run record

- **WHEN** the cron fires while `enabled=1` and no duplicate project lock exists
- **THEN** a new `runs` row exists with `trigger='schedule'`

#### Scenario: Run time is interpreted in the configured timezone

- **WHEN** `tz_offset_min=480` and `run_time='09:00'`
- **THEN** the cron job fires at 09:00 UTC+8 (01:00 UTC), not 09:00 UTC

#### Scenario: Disabled schedule does not enqueue

- **WHEN** `enabled=0` and the cron tick occurs
- **THEN** no new `runs` row is created

<!-- @trace
source: cloud-reviewer-mvp
updated: 2026-07-05
code:
  - README.md
  - backend/src/main.rs
  - frontend/src/main.ts
  - backend/src/server.rs
  - backend/src/runs.rs
  - crates/app-env/Cargo.toml
  - backend/src/projects.rs
  - frontend/src/types.ts
  - frontend/src/app.ts
  - backend/Cargo.toml
  - docs/idea/schema.md
  - Cargo.toml
  - frontend/src/api.ts
  - backend/migrations/001_initial.sql
  - .env.example
  - frontend/index.html
  - backend/src/state.rs
  - frontend/public/favicon.svg
  - crates/app-env/src/lib.rs
  - docs/idea/spec.md
  - backend/src/reports.rs
  - backend/src/schedule.rs
  - frontend/src/assets/typescript.svg
  - frontend/src/assets/hero.png
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/error.rs
  - backend/src/lib.rs
  - frontend/src/assets/vite.svg
  - frontend/src/style.css
  - frontend/vite.config.ts
  - backend/src/executor.rs
  - frontend/package.json
  - backend/src/summary.rs
  - projects.yaml
  - frontend/public/icons.svg
  - backend/src/db.rs
  - backend/src/worker.rs
  - frontend/tsconfig.json
  - skills/reviewer-batch/output-contract.md
  - backend/src/config.rs
tests:
  - backend/tests/fixtures/slow_executor.cmd
  - backend/tests/runs_execution.rs
  - backend/tests/scheduling.rs
  - backend/tests/project_config.rs
  - backend/tests/foundation.rs
  - backend/tests/report_reader.rs
-->

---
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


<!-- @trace
source: schedule-settings-catchup
updated: 2026-07-11
code:
  - frontend/src/api.ts
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - frontend/src/style.css
  - .kiro/skills/spectra-commit/SKILL.md
  - backend/src/reports.rs
  - .kiro/prompts/spectra-propose.prompt.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/error.rs
  - backend/migrations/011_runs_filter_indexes.sql
  - backend/src/person_trends.rs
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/migrations/010_pending_items_indexes.sql
  - docs/idea/roadmap-workflow-growth.md
  - .kiro/prompts/spectra-debug.prompt.md
  - .kiro/prompts/spectra-discuss.prompt.md
  - .spectra.yaml
  - backend/src/identity.rs
  - .kiro/prompts/spectra-ingest.prompt.md
  - backend/src/lib.rs
  - .kiro/skills/spectra-drift/SKILL.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/skills/spectra-apply/SKILL.md
  - backend/src/runs.rs
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/skills/spectra-propose/SKILL.md
  - frontend/src/app.ts
  - .kiro/prompts/spectra-ask.prompt.md
  - .kiro/prompts/spectra-archive.prompt.md
  - README.md
  - backend/src/summary.rs
  - docs/idea/schema.md
  - .kiro/prompts/spectra-commit.prompt.md
  - .kiro/prompts/spectra-audit.prompt.md
  - backend/src/schedule.rs
  - .kiro/skills/spectra-discuss/SKILL.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - backend/src/server.rs
tests:
  - backend/tests/dashboard.rs
  - backend/tests/report_reader.rs
  - backend/tests/pending_items.rs
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
-->

---
### Requirement: Dashboard schedule panel edits schedule settings

The dashboard schedule panel SHALL allow editing the fields supported by `PATCH /api/schedule` (except `cadence`, which MUST be shown as read-only weekly).

After a successful save, the UI MUST inform the operator that changes affecting cron registration require restarting `reviewer-server`, while `per_project_timeout_sec` and `max_concurrency` apply to the next run without restart.

#### Scenario: Save schedule from dashboard

- **WHEN** a manager updates weekday and MR poll interval on the dashboard and saves
- **THEN** the client calls `PATCH /api/schedule` with those fields
- **AND** on success the panel shows the updated labels and a restart notice for cron-related fields


<!-- @trace
source: schedule-settings-catchup
updated: 2026-07-11
code:
  - frontend/src/api.ts
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - frontend/src/style.css
  - .kiro/skills/spectra-commit/SKILL.md
  - backend/src/reports.rs
  - .kiro/prompts/spectra-propose.prompt.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/error.rs
  - backend/migrations/011_runs_filter_indexes.sql
  - backend/src/person_trends.rs
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/migrations/010_pending_items_indexes.sql
  - docs/idea/roadmap-workflow-growth.md
  - .kiro/prompts/spectra-debug.prompt.md
  - .kiro/prompts/spectra-discuss.prompt.md
  - .spectra.yaml
  - backend/src/identity.rs
  - .kiro/prompts/spectra-ingest.prompt.md
  - backend/src/lib.rs
  - .kiro/skills/spectra-drift/SKILL.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/skills/spectra-apply/SKILL.md
  - backend/src/runs.rs
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/skills/spectra-propose/SKILL.md
  - frontend/src/app.ts
  - .kiro/prompts/spectra-ask.prompt.md
  - .kiro/prompts/spectra-archive.prompt.md
  - README.md
  - backend/src/summary.rs
  - docs/idea/schema.md
  - .kiro/prompts/spectra-commit.prompt.md
  - .kiro/prompts/spectra-audit.prompt.md
  - backend/src/schedule.rs
  - .kiro/skills/spectra-discuss/SKILL.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - backend/src/server.rs
tests:
  - backend/tests/dashboard.rs
  - backend/tests/report_reader.rs
  - backend/tests/pending_items.rs
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
-->

---
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


<!-- @trace
source: schedule-settings-catchup
updated: 2026-07-11
code:
  - frontend/src/api.ts
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - frontend/src/style.css
  - .kiro/skills/spectra-commit/SKILL.md
  - backend/src/reports.rs
  - .kiro/prompts/spectra-propose.prompt.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/error.rs
  - backend/migrations/011_runs_filter_indexes.sql
  - backend/src/person_trends.rs
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/migrations/010_pending_items_indexes.sql
  - docs/idea/roadmap-workflow-growth.md
  - .kiro/prompts/spectra-debug.prompt.md
  - .kiro/prompts/spectra-discuss.prompt.md
  - .spectra.yaml
  - backend/src/identity.rs
  - .kiro/prompts/spectra-ingest.prompt.md
  - backend/src/lib.rs
  - .kiro/skills/spectra-drift/SKILL.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/skills/spectra-apply/SKILL.md
  - backend/src/runs.rs
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/skills/spectra-propose/SKILL.md
  - frontend/src/app.ts
  - .kiro/prompts/spectra-ask.prompt.md
  - .kiro/prompts/spectra-archive.prompt.md
  - README.md
  - backend/src/summary.rs
  - docs/idea/schema.md
  - .kiro/prompts/spectra-commit.prompt.md
  - .kiro/prompts/spectra-audit.prompt.md
  - backend/src/schedule.rs
  - .kiro/skills/spectra-discuss/SKILL.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - backend/src/server.rs
tests:
  - backend/tests/dashboard.rs
  - backend/tests/report_reader.rs
  - backend/tests/pending_items.rs
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
-->

---
### Requirement: Operator can confirm weekly catch-up run

The backend SHALL expose `POST /api/schedule/catch-up` that enqueues the same all-projects weekly batch pipeline as `manual_all` (creating a `runs` row the worker can execute).

On success the response MUST identify the created `run_id` (HTTP 202 or the project's existing create-run success shape).

If a conflicting in-flight run prevents enqueue, the response MUST be HTTP 409.

The dashboard SHALL show a banner when `missed_weekly_run` is non-null, with actions to confirm catch-up or dismiss for the current browser tab session only (`sessionStorage`, keyed by `due_at`). Dismiss MUST NOT persist in the database. After dismiss, a reload in the same tab MUST keep the banner hidden for that `due_at`. Closing the tab (or using a different tab) MUST show the banner again if the window is still missed. A new missed window with a different `due_at` MUST show the banner again even in the same tab.

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

#### Scenario: Session dismiss survives same-tab reload

- **GIVEN** the manager dismissed the banner for a given `due_at` via `sessionStorage`
- **WHEN** the same browser tab reloads and the window is still missed with the same `due_at`
- **THEN** the banner remains hidden

#### Scenario: New tab shows banner again after dismiss

- **GIVEN** the manager dismissed the banner for a given `due_at` in one tab
- **WHEN** the manager opens the dashboard in a new tab and the window is still missed
- **THEN** the banner is shown again

<!-- @trace
source: schedule-settings-catchup
updated: 2026-07-11
code:
  - frontend/src/api.ts
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - frontend/src/style.css
  - .kiro/skills/spectra-commit/SKILL.md
  - backend/src/reports.rs
  - .kiro/prompts/spectra-propose.prompt.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/error.rs
  - backend/migrations/011_runs_filter_indexes.sql
  - backend/src/person_trends.rs
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/migrations/010_pending_items_indexes.sql
  - docs/idea/roadmap-workflow-growth.md
  - .kiro/prompts/spectra-debug.prompt.md
  - .kiro/prompts/spectra-discuss.prompt.md
  - .spectra.yaml
  - backend/src/identity.rs
  - .kiro/prompts/spectra-ingest.prompt.md
  - backend/src/lib.rs
  - .kiro/skills/spectra-drift/SKILL.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/skills/spectra-apply/SKILL.md
  - backend/src/runs.rs
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/skills/spectra-propose/SKILL.md
  - frontend/src/app.ts
  - .kiro/prompts/spectra-ask.prompt.md
  - .kiro/prompts/spectra-archive.prompt.md
  - README.md
  - backend/src/summary.rs
  - docs/idea/schema.md
  - .kiro/prompts/spectra-commit.prompt.md
  - .kiro/prompts/spectra-audit.prompt.md
  - backend/src/schedule.rs
  - .kiro/skills/spectra-discuss/SKILL.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - backend/src/server.rs
tests:
  - backend/tests/dashboard.rs
  - backend/tests/report_reader.rs
  - backend/tests/pending_items.rs
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
-->

---
### Requirement: MR poll cron triggers scheduled scans on an independent interval

The backend SHALL register a second cron job at startup (independent of the weekly batch cron) that fires every `schedule_config.mr_poll_interval_min` minutes. On each firing, the job MUST create a `runs` row with `trigger='mr_poll'`, insert one `run_projects` row per project with `is_git_repo=1` and `state='queued'`, and enqueue work for the worker pool, applying the same per-project deduplication lock used by the weekly batch (a project already `queued` or `running` under any active run MUST NOT be enqueued again).

If `schedule_config.enabled` is `0`, the MR poll cron MUST still be registered and fire independently of the weekly batch enabled flag, because the two tracks have separate operational cadences.

#### Scenario: MR poll fires on its configured interval

- **WHEN** `mr_poll_interval_min` is `60` and one hour elapses with the scheduler running
- **THEN** a new `runs` row with `trigger='mr_poll'` is created covering all healthy projects

#### Scenario: MR poll skips a project already locked by the weekly track

- **WHEN** the MR poll cron fires while a project has an active `run_projects` row with `state='running'` from a `trigger='schedule'` run
- **THEN** that project is not enqueued into the new `mr_poll` run, and other healthy projects are still enqueued

<!-- @trace
source: mr-review-track
updated: 2026-07-17
code:
  - .spectra.yaml
  - backend/src/server.rs
  - .kiro/prompts/spectra-commit.prompt.md
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/main.tsx
  - backend/src/lib.rs
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - backend/src/schedule.rs
  - frontend/src/hooks/useApi.ts
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/lib/icons.ts
  - .kiro/skills/spectra-drift/SKILL.md
  - skills/scan-mrs-headless/WORKFLOW.md
  - .kiro/prompts/spectra-audit.prompt.md
  - frontend/src/lib/format.ts
  - frontend/package.json
  - frontend/vite.config.ts
  - frontend/src/components/ui/Input.tsx
  - backend/src/report_chat.rs
  - .kiro/skills/spectra-archive/SKILL.md
  - frontend/src/components/ui/Card.tsx
  - .kiro/skills/spectra-apply/SKILL.md
  - frontend/src/components/ui/Avatar.tsx
  - backend/src/state.rs
  - .kiro/skills/spectra-commit/SKILL.md
  - skills/scan-mrs-headless/observation-guidelines.md
  - frontend/src/pages/MrInboxPage.tsx
  - backend/src/pending_items.rs
  - frontend/src/components/ui/Button.tsx
  - scripts/triage-mrs.py
  - frontend/src/style.css
  - .kiro/prompts/spectra-propose.prompt.md
  - backend/src/projects.rs
  - frontend/tsconfig.json
  - backend/src/worktree.rs
  - skills/reviewer-batch/WORKFLOW.md
  - .kiro/prompts/spectra-archive.prompt.md
  - backend/migrations/006_mr_review_agent_session.sql
  - backend/src/person_trends.rs
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Tabs.tsx
  - .kiro/skills/spectra-discuss/SKILL.md
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/pages/DashboardPage.tsx
  - backend/migrations/012_pending_open_by_project.sql
  - docs/idea/roadmap-workflow-growth.md
  - .kiro/skills/spectra-debug/SKILL.md
  - backend/src/worker.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/components/layout/Toast.tsx
  - backend/Cargo.toml
  - frontend/src/types.ts
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-audit/SKILL.md
  - frontend/src/index.css
  - frontend/src/lib/catchup.ts
  - frontend/src/lib/tokens.ts
  - frontend/src/components/ui/NavItem.tsx
  - README.md
  - frontend/index.html
  - frontend/src/components/layout/Sidebar.tsx
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/src/runs.rs
  - .kiro/prompts/spectra-apply.prompt.md
  - backend/src/summary.rs
  - frontend/src/App.tsx
  - frontend/src/components/ui/index.ts
  - frontend/src/components/ui/StatCard.tsx
  - skills/scan-mrs-headless/output-contract.md
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/src/app.ts
  - frontend/src/components/ui/Badge.tsx
  - .kiro/prompts/spectra-debug.prompt.md
  - frontend/src/pages/PeoplePage.tsx
  - .kiro/prompts/spectra-discuss.prompt.md
  - frontend/src/components/ui/ConfirmDialog.tsx
  - frontend/src/main.ts
  - docs/idea/schema.md
  - backend/migrations/011_runs_filter_indexes.sql
  - backend/src/reports.rs
  - backend/src/error.rs
  - backend/migrations/013_mr_review_chat_messages.sql
  - backend/migrations/009_mr_reviews_project_status_index.sql
  - frontend/src/pages/RunsPage.tsx
  - backend/migrations/007_mr_review_project_gates.sql
  - skills/project-adr-notes/SKILL.md
  - .kiro/prompts/spectra-ingest.prompt.md
  - backend/src/executor.rs
  - frontend/src/pages/ProjectsPage.tsx
  - backend/src/config.rs
  - backend/src/identity.rs
  - backend/migrations/014_person_report_chat.sql
  - .kiro/prompts/spectra-ask.prompt.md
  - backend/migrations/008_mr_scan_force.sql
  - backend/src/mr_reviews.rs
  - backend/migrations/010_pending_items_indexes.sql
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-ask/SKILL.md
  - docs/design_handoff_reviewer_redesign/support.js
  - .kiro/skills/spectra-propose/SKILL.md
  - frontend/src/context/ToastContext.tsx
  - backend/src/mr_change_materials.rs
  - frontend/src/api.ts
tests:
  - backend/tests/foundation.rs
  - backend/tests/fixtures/flood_stdout.sh
  - backend/tests/fixtures/write_draft_then_hang.sh
  - backend/tests/fixtures/fake_triage_eligible.py
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - backend/tests/fixtures/report_chat_fail.cmd
  - frontend/src/hooks/useApi.test.ts
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/write_draft_then_hang.cmd
  - backend/tests/identity.rs
  - backend/tests/fixtures/write_draft_then_hang.py
  - backend/tests/fixtures/agent_turn_fail.sh
  - frontend/src/theme.test.ts
  - frontend/src/pages/MrInboxPage.test.tsx
  - backend/tests/fixtures/flood_stdout.cmd
  - backend/tests/schedule_api.rs
  - scripts/test_triage_mrs.py
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/lib/catchup.test.ts
  - backend/tests/fixtures/agent_turn_fail.cmd
  - backend/tests/mr_reviews.rs
  - frontend/src/components/ui/atoms.test.tsx
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_reader.rs
  - backend/tests/executor_cancellation.rs
  - backend/tests/fixtures/slow_executor.sh
  - frontend/src/pages/RunsPage.test.tsx
  - backend/tests/person_trends.rs
  - backend/tests/pending_items.rs
  - frontend/src/App.routes.test.tsx
  - backend/tests/fixtures/agent_turn_ok.py
  - backend/tests/graceful_shutdown.rs
  - backend/tests/runs_execution.rs
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/fixtures/agent_turn_ok.sh
  - frontend/src/components/layout/Toast.test.tsx
  - backend/tests/fixtures/agent_turn_ok.cmd
  - backend/tests/scheduling.rs
  - backend/tests/fixtures/flood_stdout.py
  - frontend/src/test/setup.ts
  - backend/tests/dashboard.rs
-->