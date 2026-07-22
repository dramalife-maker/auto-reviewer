# reviewer-execution Specification

## Purpose

TBD - created by archiving change 'cloud-reviewer-mvp'. Update Purpose after archive.

## Requirements

### Requirement: Manual batch run enqueues all projects

The backend SHALL expose `POST /api/runs` accepting JSON `{ "trigger": "manual_all" }`.

The handler MUST create a `runs` row with `trigger='manual_all'` and `status='running'`, insert one `run_projects` row per project with `state='queued'`, and enqueue work for the worker pool.

If a project already has a `run_projects` row with `state` in `('queued','running')` for any active run, the server MUST reject the new batch with HTTP 409.

#### Scenario: Start manual batch run

- **WHEN** a client posts `{ "trigger": "manual_all" }` and no project is already queued or running
- **THEN** the response includes a run id and all projects appear in `run_projects` with `state='queued'`


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
### Requirement: Worker executes reviewer skill subprocess per project

For each dequeued project, the worker SHALL set the subprocess working directory to the target worktree path supplied by the `repo-worktree` capability — the resident worktree of the first `default_branches` entry for a weekly batch, or the merge-request worktree for an MR review — rather than to `projects.repo_path` directly. The worker SHALL execute the configured reviewer agent with the configured reviewer-batch prompt and enforce timeout using `schedule_config.per_project_timeout_sec`.

If the worktree cannot be supplied (for example, the branch was deleted on the remote or fetch failed after retries), the worker MUST NOT start the subprocess for that project and MUST record the failure without aborting remaining projects.

On timeout, the worker MUST kill the subprocess, set `run_projects.state='skipped_timeout'`, and continue remaining projects.

On success, the worker MUST set `run_projects.state='done'` and record `duration_sec`. After a successful subprocess exit, person-level files under `reports/_people/{display_name}/` are maintained by the subprocess per workflow contract (the worker does not parse person-level files into SQLite).

#### Scenario: Project completes within timeout

- **WHEN** the subprocess exits with code 0 before timeout
- **THEN** the corresponding `run_projects.state` becomes `done`

#### Scenario: Project exceeds timeout

- **WHEN** the subprocess runs longer than `per_project_timeout_sec`
- **THEN** the subprocess is terminated and `run_projects.state` becomes `skipped_timeout`

#### Scenario: Subprocess runs inside the supplied worktree

- **WHEN** a project is dequeued and its target worktree path is supplied
- **THEN** the subprocess working directory is that worktree path, not `projects.repo_path`

#### Scenario: Unavailable worktree skips the subprocess

- **WHEN** the target worktree cannot be supplied because the remote branch was deleted or fetch failed after retries
- **THEN** the worker does not start the subprocess and records the failure while continuing remaining projects

#### Scenario: Successful run maintains person-level files via workflow

- **GIVEN** a weekly batch subprocess completes successfully for a project with resolved author "Alice Chen"
- **WHEN** the workflow finishes
- **THEN** `reports/_people/Alice Chen/index.md` exists or is updated (append semantics per workflow)
- **AND** project-level `reports/{project}/Alice Chen/{run_date}/summary.md` exists


<!-- @trace
source: person-level-long-term-observations
updated: 2026-07-09
code:
  - frontend/src/app.ts
  - backend/src/config.rs
  - docs/idea/reviewer_project_list_with_run.html
  - backend/src/dashboard.rs
  - backend/src/schedule.rs
  - docs/idea/reviewer_project_settings.html
  - README.md
  - frontend/src/api.ts
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - frontend/src/style.css
  - backend/src/person_trends.rs
  - backend/src/error.rs
  - backend/migrations/004_project_settings.sql
  - .env.example
  - frontend/src/types.ts
  - docs/idea/schema.md
  - backend/src/runs.rs
  - docs/idea/spec.md
  - backend/src/worker.rs
  - backend/Cargo.toml
  - backend/src/projects.rs
  - backend/migrations/005_drop_gitlab_project_id.sql
  - backend/src/lib.rs
  - docs/idea/migration-person-observations.md
  - frontend/index.html
  - docs/idea/reviewer_app_dashboard_home.html
  - backend/src/executor.rs
tests:
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
  - backend/tests/project_config.rs
  - backend/tests/dashboard.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Summary files are parsed into reports and pending items

After a successful project run, the backend SHALL scan `$DATA_ROOT_DIR/reports/<name>/<person>/<YYYY-MM-DD>/summary.md` files produced by the skill.

For each summary file, the parser MUST read YAML frontmatter fields `person`, `project`, `date`, `one_line`, `mr_count`, `commit_count`, resolve `person_id` by matching `people.display_name` to frontmatter `person`, upsert `reports` for `(project_id, person_id, report_date)`, and insert `pending_items` for each bullet under heading `## 待確認`.

If frontmatter `person` does not match any existing `people.display_name`, the parser MUST skip that summary file and MUST NOT create a new `people` row.

#### Scenario: Parse summary with two pending questions

- **WHEN** a summary file contains frontmatter and two bullets under `## 待確認` and `person` matches an existing person
- **THEN** one `reports` row exists and two `pending_items` rows exist with `status='open'`

##### Example: frontmatter and pending bullets

- **GIVEN** summary frontmatter `person: Alice`, `date: 2026-07-05`, `one_line: Stable week` and a `people` row with `display_name='Alice'`
- **WHEN** the parser processes the file with two `-` lines under `## 待確認`
- **THEN** `reports.one_line` is `Stable week` and `pending_items` count for that person is 2

#### Scenario: Unknown person in summary is skipped

- **WHEN** summary frontmatter `person` is `Ghost` and no `people` row has that display name
- **THEN** no `reports` row is created for that file


<!-- @trace
source: person-identity-resolution
updated: 2026-07-09
code:
  - frontend/src/app.ts
  - backend/src/reports.rs
  - docs/idea/migration-person-observations.md
  - backend/src/config.rs
  - backend/src/person_trends.rs
  - frontend/src/types.ts
  - frontend/src/api.ts
  - docs/idea/reviewer_project_list_with_run.html
  - backend/src/runs.rs
  - skills/reviewer-batch/output-contract.md
  - backend/migrations/005_drop_gitlab_project_id.sql
  - docs/idea/schema.md
  - backend/migrations/004_project_settings.sql
  - backend/src/executor.rs
  - docs/idea/reviewer_project_settings.html
  - backend/src/worker.rs
  - frontend/index.html
  - README.md
  - backend/src/schedule.rs
  - backend/src/server.rs
  - backend/src/summary.rs
  - backend/src/identity.rs
  - backend/Cargo.toml
  - .env.example
  - backend/src/error.rs
  - backend/src/lib.rs
  - frontend/src/style.css
  - backend/src/dashboard.rs
  - docs/idea/spec.md
  - skills/reviewer-batch/WORKFLOW.md
  - docs/idea/reviewer_app_dashboard_home.html
  - backend/src/projects.rs
tests:
  - backend/tests/report_reader.rs
  - backend/tests/dashboard.rs
  - backend/tests/runs_execution.rs
  - backend/tests/person_trends.rs
  - backend/tests/project_config.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Weekly manifest includes resolved authors

Before spawning the reviewer-batch subprocess, the backend SHALL write `manifest.json` including an `authors` array. Each element MUST contain `email` (normalized git author email), `git_name` (raw `%an`), `person_id` (integer), and `display_name` (canonical `people.display_name`).

Only authors with a resolved `person_id` MUST appear in `authors`. Unresolved authors MUST NOT appear in the array.

#### Scenario: Manifest lists only resolved authors

- **WHEN** a project has commits from `alice@co.com` (bound) and `bob@gmail.com` (unbound) in the analysis window
- **THEN** manifest `authors` contains only the entry for Alice

##### Example: manifest authors shape

- **GIVEN** person id 1 with `display_name` "Alice Chen" bound to `git_email: alice@co.com`
- **WHEN** the weekly manifest is written for that project
- **THEN** `authors` contains `{ "email": "alice@co.com", "git_name": "Alice", "person_id": 1, "display_name": "Alice Chen" }`


<!-- @trace
source: person-identity-resolution
updated: 2026-07-09
code:
  - frontend/src/app.ts
  - backend/src/reports.rs
  - docs/idea/migration-person-observations.md
  - backend/src/config.rs
  - backend/src/person_trends.rs
  - frontend/src/types.ts
  - frontend/src/api.ts
  - docs/idea/reviewer_project_list_with_run.html
  - backend/src/runs.rs
  - skills/reviewer-batch/output-contract.md
  - backend/migrations/005_drop_gitlab_project_id.sql
  - docs/idea/schema.md
  - backend/migrations/004_project_settings.sql
  - backend/src/executor.rs
  - docs/idea/reviewer_project_settings.html
  - backend/src/worker.rs
  - frontend/index.html
  - README.md
  - backend/src/schedule.rs
  - backend/src/server.rs
  - backend/src/summary.rs
  - backend/src/identity.rs
  - backend/Cargo.toml
  - .env.example
  - backend/src/error.rs
  - backend/src/lib.rs
  - frontend/src/style.css
  - backend/src/dashboard.rs
  - docs/idea/spec.md
  - skills/reviewer-batch/WORKFLOW.md
  - docs/idea/reviewer_app_dashboard_home.html
  - backend/src/projects.rs
tests:
  - backend/tests/report_reader.rs
  - backend/tests/dashboard.rs
  - backend/tests/runs_execution.rs
  - backend/tests/person_trends.rs
  - backend/tests/project_config.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Reviewer-batch workflow uses manifest authors

The reviewer-batch workflow SHALL determine the set of engineers to report on exclusively from `manifest.authors`. It MUST NOT enumerate git authors independently to decide person groupings.

For each `authors[]` entry, report files MUST be written under `{report_root}/{display_name}/{run_date}/`.

#### Scenario: Workflow skips git log person discovery

- **WHEN** manifest `authors` contains one entry with `display_name` "Alice Chen"
- **THEN** the workflow produces reports only under `Alice Chen/` and does not create directories for other git display names

<!-- @trace
source: person-identity-resolution
updated: 2026-07-09
code:
  - frontend/src/app.ts
  - backend/src/reports.rs
  - docs/idea/migration-person-observations.md
  - backend/src/config.rs
  - backend/src/person_trends.rs
  - frontend/src/types.ts
  - frontend/src/api.ts
  - docs/idea/reviewer_project_list_with_run.html
  - backend/src/runs.rs
  - skills/reviewer-batch/output-contract.md
  - backend/migrations/005_drop_gitlab_project_id.sql
  - docs/idea/schema.md
  - backend/migrations/004_project_settings.sql
  - backend/src/executor.rs
  - docs/idea/reviewer_project_settings.html
  - backend/src/worker.rs
  - frontend/index.html
  - README.md
  - backend/src/schedule.rs
  - backend/src/server.rs
  - backend/src/summary.rs
  - backend/src/identity.rs
  - backend/Cargo.toml
  - .env.example
  - backend/src/error.rs
  - backend/src/lib.rs
  - frontend/src/style.css
  - backend/src/dashboard.rs
  - docs/idea/spec.md
  - skills/reviewer-batch/WORKFLOW.md
  - docs/idea/reviewer_app_dashboard_home.html
  - backend/src/projects.rs
tests:
  - backend/tests/report_reader.rs
  - backend/tests/dashboard.rs
  - backend/tests/runs_execution.rs
  - backend/tests/person_trends.rs
  - backend/tests/project_config.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Weekly batch manifest includes analysis window and authors

The worker SHALL write `manifest.json` under `{DATA_ROOT_DIR}/runs/{run_id}/projects/{project_id}/manifest.json` before spawning the reviewer subprocess.

The manifest MUST include fields: `mode`, `project_name`, `repo_path`, `report_root`, `run_date`, `since`, `output_contract`, and `authors`.

Each `authors[]` entry MUST include `email`, `git_name`, `person_id`, and `display_name` for resolved persons only.

The manifest MUST include `person_report_root` set to `{DATA_ROOT_DIR}/reports/_people` (the shared person-level reports root; workflow resolves per-author paths as `{person_report_root}/{display_name}/`).

#### Scenario: Manifest includes person report root

- **WHEN** a weekly batch run prepares manifest for a project with at least one resolved author
- **THEN** `manifest.json` contains `person_report_root` pointing to `reports/_people` under `DATA_ROOT_DIR`


<!-- @trace
source: person-level-long-term-observations
updated: 2026-07-09
code:
  - frontend/src/app.ts
  - backend/src/config.rs
  - docs/idea/reviewer_project_list_with_run.html
  - backend/src/dashboard.rs
  - backend/src/schedule.rs
  - docs/idea/reviewer_project_settings.html
  - README.md
  - frontend/src/api.ts
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - frontend/src/style.css
  - backend/src/person_trends.rs
  - backend/src/error.rs
  - backend/migrations/004_project_settings.sql
  - .env.example
  - frontend/src/types.ts
  - docs/idea/schema.md
  - backend/src/runs.rs
  - docs/idea/spec.md
  - backend/src/worker.rs
  - backend/Cargo.toml
  - backend/src/projects.rs
  - backend/migrations/005_drop_gitlab_project_id.sql
  - backend/src/lib.rs
  - docs/idea/migration-person-observations.md
  - frontend/index.html
  - docs/idea/reviewer_app_dashboard_home.html
  - backend/src/executor.rs
tests:
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
  - backend/tests/project_config.rs
  - backend/tests/dashboard.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Dashboard includes recent runs

The backend SHALL include `recent_runs` on `GET /api/dashboard` as an array of up to five run list items (same fields as `GET /api/runs` list items), ordered by `started_at` descending.

When no runs exist, `recent_runs` MUST be an empty array.

#### Scenario: Dashboard returns latest five runs

- **GIVEN** more than five runs exist
- **WHEN** a client calls `GET /api/dashboard`
- **THEN** `recent_runs` contains exactly five items
- **AND** they are the newest by `started_at`

<!-- @trace
source: run-history-observability
updated: 2026-07-11
code:
  - docs/idea/schema.md
  - frontend/src/types.ts
  - .kiro/prompts/spectra-debug.prompt.md
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/src/summary.rs
  - backend/src/runs.rs
  - .kiro/prompts/spectra-commit.prompt.md
  - .kiro/skills/spectra-discuss/SKILL.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/prompts/spectra-propose.prompt.md
  - backend/src/person_trends.rs
  - frontend/src/api.ts
  - .kiro/prompts/spectra-ask.prompt.md
  - frontend/src/app.ts
  - .kiro/skills/spectra-commit/SKILL.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/lib.rs
  - .kiro/skills/spectra-drift/SKILL.md
  - README.md
  - backend/src/error.rs
  - backend/src/reports.rs
  - backend/src/identity.rs
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/prompts/spectra-archive.prompt.md
  - backend/src/server.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - docs/idea/roadmap-workflow-growth.md
  - backend/migrations/010_pending_items_indexes.sql
  - .kiro/prompts/spectra-audit.prompt.md
  - .spectra.yaml
  - backend/src/pending_items.rs
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-propose/SKILL.md
  - .kiro/skills/spectra-apply/SKILL.md
  - frontend/src/style.css
  - .kiro/prompts/spectra-discuss.prompt.md
  - .kiro/prompts/spectra-ingest.prompt.md
tests:
  - backend/tests/pending_items.rs
  - backend/tests/identity.rs
  - backend/tests/dashboard.rs
  - backend/tests/report_reader.rs
  - backend/tests/person_trends.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: Weekly batch manifest includes open pending items

Before spawning the reviewer-batch subprocess, the backend MUST include an `open_pending` array on the weekly `manifest.json` for that project.

Each element MUST contain: `id` (pending item id), `person_id`, `display_name` (canonical `people.display_name`), and `question`.

The array MUST contain every `pending_items` row for that `project_id` with `status='open'`, ordered stably by `person_id` ascending then `id` ascending.

When no open pending rows exist for the project, `open_pending` MUST be an empty array.

#### Scenario: Manifest lists open pending for the project

- **GIVEN** project G has an open pending item for person Alice with question `Why choose A?`
- **AND** project G has a resolved pending item with a different question
- **WHEN** the weekly manifest is written for project G
- **THEN** `open_pending` contains exactly one element for Alice with that question and its numeric `id`
- **AND** the resolved item is omitted

##### Example: open pending shape

- **GIVEN** open row `id=7`, `person_id=1`, `display_name="Alice Chen"`, `question="Why choose A?"`
- **WHEN** the weekly manifest is written
- **THEN** `open_pending` includes `{ "id": 7, "person_id": 1, "display_name": "Alice Chen", "question": "Why choose A?" }`

#### Scenario: Empty open pending when none exist

- **GIVEN** project G has no open pending items
- **WHEN** the weekly manifest is written for project G
- **THEN** `open_pending` is an empty array


<!-- @trace
source: manifest-open-pending-reuse
updated: 2026-07-11
code:
  - backend/src/server.rs
  - frontend/src/style.css
  - backend/src/summary.rs
  - backend/src/dashboard.rs
  - skills/reviewer-batch/output-contract.md
  - README.md
  - backend/migrations/012_pending_open_by_project.sql
  - frontend/src/api.ts
  - skills/reviewer-batch/WORKFLOW.md
  - frontend/src/types.ts
  - backend/src/schedule.rs
  - backend/src/runs.rs
  - docs/idea/schema.md
  - frontend/src/app.ts
  - backend/src/error.rs
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/schedule_api.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Reviewer-batch reuses open pending question text verbatim

The reviewer-batch workflow MUST read `manifest.open_pending` when composing `## 待確認` for each author.

For each `open_pending` entry that matches the current author by `person_id` or `display_name`, the workflow MUST choose exactly one of the following for this run's `## 待確認`:

1. Include a bullet whose text is exactly equal to that entry's `question` (no paraphrase or rewording), or
2. Omit that question from `## 待確認` entirely when the issue is no longer relevant for this run.

Omitting an open question MUST NOT resolve it in the database (the worker and workflow do not write SQLite).

New questions that do not correspond to any `open_pending` entry for that author MUST be allowed as additional bullets under the usual 0–5 limit, using new wording only for genuinely new issues.

#### Scenario: Continuing open issue keeps exact wording

- **GIVEN** `open_pending` contains `{ "display_name": "Alice Chen", "question": "Why choose A?" }`
- **AND** the issue remains relevant this week
- **WHEN** the workflow writes Alice Chen's `summary.md`
- **THEN** `## 待確認` includes a bullet whose text is exactly `Why choose A?`

#### Scenario: Stale open issue omitted from summary

- **GIVEN** `open_pending` contains an open question for Alice Chen
- **AND** the workflow judges the issue no longer relevant this week
- **WHEN** the workflow writes Alice Chen's `summary.md`
- **THEN** that question is absent from `## 待確認`
- **AND** no database resolve is performed by the workflow

<!-- @trace
source: manifest-open-pending-reuse
updated: 2026-07-11
code:
  - backend/src/server.rs
  - frontend/src/style.css
  - backend/src/summary.rs
  - backend/src/dashboard.rs
  - skills/reviewer-batch/output-contract.md
  - README.md
  - backend/migrations/012_pending_open_by_project.sql
  - frontend/src/api.ts
  - skills/reviewer-batch/WORKFLOW.md
  - frontend/src/types.ts
  - backend/src/schedule.rs
  - backend/src/runs.rs
  - docs/idea/schema.md
  - frontend/src/app.ts
  - backend/src/error.rs
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/schedule_api.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Weekly summary includes resolved section for closed pending

The reviewer-batch `summary.md` contract MUST include a fourth level-2 heading `## 已釐清` after `## 待確認`.

Bullets under `## 已釐清` MUST use `- ` list markers. The section MUST remain valid when empty (heading present, zero bullets).

When the workflow determines that an `open_pending` entry for the current author is resolved this run, it MUST write that entry's `question` text verbatim as a bullet under `## 已釐清`, and MUST NOT also list that question under `## 待確認`.

Omitting a question from both `## 待確認` and `## 已釐清` MUST leave the corresponding open database row unchanged.

#### Scenario: Resolved open issue listed under 已釐清

- **GIVEN** `open_pending` contains `{ "display_name": "Alice Chen", "question": "Why choose A?" }`
- **AND** the workflow judges the issue resolved this week
- **WHEN** the workflow writes Alice Chen's `summary.md`
- **THEN** `## 已釐清` includes a bullet whose text is exactly `Why choose A?`
- **AND** `## 待確認` does not include that text

#### Scenario: Omitted open issue stays open at workflow layer

- **GIVEN** an open pending question for Alice Chen
- **AND** the workflow omits it from both `## 待確認` and `## 已釐清`
- **WHEN** the summary is written
- **THEN** the workflow does not write SQLite
- **AND** closure of that row is not requested by the summary file


<!-- @trace
source: summary-resolved-pending
updated: 2026-07-11
code:
  - frontend/src/types.ts
  - backend/src/runs.rs
  - backend/src/schedule.rs
  - docs/idea/schema.md
  - backend/src/dashboard.rs
  - frontend/src/app.ts
  - frontend/src/style.css
  - backend/migrations/012_pending_open_by_project.sql
  - backend/src/server.rs
  - frontend/src/api.ts
  - README.md
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/summary.rs
  - skills/reviewer-batch/output-contract.md
  - backend/src/error.rs
tests:
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: Summary ingestion auto-resolves matching open pending from 已釐清

After upserting a weekly `summary.md`, the backend MUST parse bullets under `## 已釐清`.

For each bullet text Q, if an open `pending_items` row exists for the summary's resolved `person_id`, the summary's `project_id`, and `question` exactly equal to Q, the backend MUST resolve that row using the same database fields as manual closure: `status='resolved'`, `resolved_date` set to the schedule-timezone calendar month `YYYY-MM`, and `resolution_note` left null when not provided by the summary.

The backend MUST attempt to sync the person `_notes.md` resolved-line format after a successful database resolve. If notes sync fails, the backend MUST leave the row `resolved`, MUST log a warning, and MUST continue ingesting remaining summaries.

A `## 已釐清` bullet with no matching open row MUST be ignored without failing the ingest of that summary.

#### Scenario: Exact open question in 已釐清 becomes resolved

- **GIVEN** an open pending item for person Alice, project G, question `Why choose A?`
- **WHEN** a weekly summary for Alice and project G is ingested with that exact text under `## 已釐清`
- **THEN** that pending item has `status` equal to `resolved`
- **AND** `resolved_date` matches `YYYY-MM` for the schedule timezone

#### Scenario: 待確認 omission without 已釐清 does not resolve

- **GIVEN** an open pending item for person Alice, project G, question `Why choose A?`
- **WHEN** a weekly summary for Alice and project G is ingested with empty `## 已釐清` and without that question under `## 待確認`
- **THEN** that pending item remains `status` equal to `open`

#### Scenario: Unknown 已釐清 bullet is ignored

- **GIVEN** no open pending item with question `Never seen?`
- **WHEN** a summary is ingested containing `- Never seen?` under `## 已釐清`
- **THEN** ingest succeeds
- **AND** no pending row is created solely from the 已釐清 section

<!-- @trace
source: summary-resolved-pending
updated: 2026-07-11
code:
  - frontend/src/types.ts
  - backend/src/runs.rs
  - backend/src/schedule.rs
  - docs/idea/schema.md
  - backend/src/dashboard.rs
  - frontend/src/app.ts
  - frontend/src/style.css
  - backend/migrations/012_pending_open_by_project.sql
  - backend/src/server.rs
  - frontend/src/api.ts
  - README.md
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/summary.rs
  - skills/reviewer-batch/output-contract.md
  - backend/src/error.rs
tests:
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: Weekly and MR manifests include notes_dir

Before spawning a weekly-batch or MR-poll reviewer subprocess, the backend MUST include `notes_dir` on that project’s `manifest.json`.

`notes_dir` MUST be the absolute (or data-root-normalized) path `{DATA_ROOT_DIR}/reports/{project_name}/.notes`, using the same path separator normalization as `report_root`.

The backend MUST NOT require the directory to exist when writing the manifest. Creating `.notes/` remains the writer’s responsibility on first ADR write.

#### Scenario: Weekly manifest exposes notes_dir

- **WHEN** a weekly batch run prepares a manifest for project `game-backend`
- **THEN** `manifest.json` contains `notes_dir` ending with `reports/game-backend/.notes` (after normalization)

#### Scenario: MR poll manifest exposes notes_dir

- **WHEN** an MR poll run prepares a manifest for project `game-backend`
- **THEN** `manifest.json` contains the same `notes_dir` value shape as the weekly manifest for that project


<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Agent-turn receives ADR skill and notes_dir

When the backend executes `POST /api/mr-reviews/:id/agent-turn` against a draft review with a resumable agent session, it MUST supply the project ADR skill materials from `skills/project-adr-notes/` (Claude: append-system-prompt-file equivalent; Cursor: equivalent prompt inclusion) and MUST expose the project `notes_dir` path in the turn context so Write/Read under that directory is possible.

Agent-turn ADR writes MUST NOT publish to GitLab and MUST NOT be required to modify the draft body. Draft re-read behavior after a turn remains unchanged from existing agent-turn draft handling.

#### Scenario: Agent-turn command includes ADR skill

- **WHEN** the executor builds an agent-turn command for Claude (non-stub executor)
- **THEN** the command includes the project-adr-notes skill file among appended system prompt files

#### Scenario: Turn context names notes_dir

- **WHEN** an agent-turn runs for a review belonging to project `game-backend`
- **THEN** the turn context includes `notes_dir` pointing at that project’s `.notes` directory


<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Reviewer workflows consume notes_dir

The `reviewer-batch` and `scan-mrs-headless` workflows MUST document and require: read `manifest.notes_dir` / `index.md` before adding technical-choice questions; write only under paths already allowed by their contracts plus `.notes` only when an interactive agent-turn skill applies (headless weekly/MR scan MUST NOT write ADRs in this change).

#### Scenario: Weekly workflow mentions notes_dir

- **WHEN** an implementer inspects `skills/reviewer-batch/WORKFLOW.md`
- **THEN** the workflow states that `manifest.notes_dir` MUST be read before composing new technical-choice `## 待確認` items

#### Scenario: MR scan workflow mentions notes_dir

- **WHEN** an implementer inspects `skills/scan-mrs-headless/WORKFLOW.md`
- **THEN** the workflow states that `manifest.notes_dir` MUST be read before composing suggested technical-choice follow-ups

<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Reviewer executors honor shutdown cancellation

Weekly batch, MR scan, and agent-turn executor paths MUST accept a shared cancellation token (or equivalent cooperative cancel signal from process shutdown). While waiting on a reviewer subprocess, the executor MUST race the wait against cancellation. If cancellation wins, the executor MUST kill the subprocess process tree using the same kill semantics as timeout handling, and MUST return a failure outcome whose error identifies shutdown interruption (not a timeout skip).

HTTP agent-turn handlers MUST use the same cancellation token from application state so an in-flight clarification turn is cancelled during process shutdown.

#### Scenario: Weekly or MR executor fails on cancel

- **WHEN** `execute_weekly_batch` or `execute_mr_review` is waiting on a child and the shutdown cancellation token fires
- **THEN** the child process tree is killed and the function returns a failed outcome (not `skipped_timeout`) with an error identifying shutdown interruption

#### Scenario: Agent-turn honors the same token

- **WHEN** an HTTP agent-turn is waiting on a child and process shutdown cancels the shared token
- **THEN** the child process tree is killed and the turn fails without leaving an orphaned reviewer process

<!-- @trace
source: graceful-shutdown
updated: 2026-07-17
code:
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/reports.rs
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/worker.rs
  - frontend/src/types.ts
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: Manual MR scan enqueues a single project

The backend SHALL expose `POST /api/projects/:id/mr-scan`. The handler MUST create a `runs` row with `trigger='manual_mr_poll'` and `status='running'`, insert one `run_projects` row for the target project with `state='queued'`, and enqueue work for the worker pool using the `mr_poll` manifest mode and the `scan-mrs-headless` workflow.

If the target project already has a `run_projects` row with `state` in `('queued','running')` for any active run (from either track), the server MUST reject the request with HTTP 409.

#### Scenario: Manual MR scan starts for an idle project

- **WHEN** a client posts `POST /api/projects/5/mr-scan` and project 5 has no queued or running `run_projects` row
- **THEN** the response includes a run id and project 5 appears in `run_projects` with `state='queued'` under a `runs` row with `trigger='manual_mr_poll'`

#### Scenario: Manual MR scan is rejected while a weekly run is in progress

- **WHEN** a client posts `POST /api/projects/5/mr-scan` while project 5 already has a `run_projects` row with `state='running'` under a `trigger='manual_project'` run
- **THEN** the server responds with HTTP 409 and no new run is created


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

---
### Requirement: MR scan subprocess enables agent session persistence

When executing an MR scan (`mr_poll` or `manual_mr_poll`), the backend SHALL run `scripts/triage-mrs.py` first, then spawn one headless agent subprocess **per** entry in `eligible_mrs.json`, **sequentially** (MR N+1 MUST NOT start until MR N's agent subprocess has finished — one agent at a time within the project). Each subprocess MUST use `stream-json` output and MUST enable provider session persistence (Claude: MUST NOT pass `--no-session-persistence`; Cursor: default session behavior).

The worker MUST atomically claim each `run_projects` row when dequeuing (transition `queued` → `running` in the same step as selecting the next job) so a single queued project cannot be spawned more than once.

The executor MUST capture stdout from each per-MR subprocess and make it available to the MR draft ingestion step for `session_id` extraction. Weekly batch execution (`schedule`, `manual_all`, `manual_project`) MUST continue to use `--no-session-persistence` for Claude and MUST NOT change behavior.

The scan-mrs-headless workflow MUST NOT invoke `glab mr list` for merge request discovery; triage is complete before the agent starts. Before each per-MR agent subprocess, the worker MUST precompute change materials inside the provisioned MR worktree against `origin/<target_branch>...HEAD` (`git fetch`, `git log --oneline`, `git diff --stat`, and a size-capped `git diff` soft-capped so a single tool Read can consume the overview), write them under the run project directory, and expose absolute paths on the MR manifest as `change_log_path`, `change_stat_path`, and `change_diff_path`. The workflow MUST gather the change set primarily from `change_stat` / `change_log`, MUST NOT page through the entire `change.diff`, and MUST prefer Reading a bounded set of changed source files in the worktree. The workflow MUST NOT invoke `glab mr diff`. The workflow MUST NOT re-run a full `git fetch` / `git log origin/<target_branch>...HEAD` / `git diff origin/<target_branch>...HEAD` when the precomputed paths are present; it MAY run a single-path `git diff origin/<target_branch>...HEAD -- <path>` when needed. The workflow MAY invoke `glab mr view` for the single `mr_iid` specified in the manifest when gathering discussion / description context that is not available from git. The workflow MUST write the MR draft before the observation snippet and MUST NOT broadly Glob `pending_dir` or `reports/`.

#### Scenario: Eligible MRs in one project run one agent at a time

- **WHEN** triage writes `eligible` with MR iids 42 and 55 for a single project
- **THEN** the worker spawns exactly two agent subprocesses and the second MUST start only after the first has exited

#### Scenario: Worker precomputes change materials before the agent

- **WHEN** an eligible MR with non-empty `target_branch` is about to be reviewed
- **THEN** the worker writes `change_log.txt`, `change_stat.txt`, and `change.diff` for `origin/<target_branch>...HEAD` and the per-MR manifest includes `change_log_path`, `change_stat_path`, and `change_diff_path`

#### Scenario: Queued run project is claimed at most once

- **WHEN** the worker drain loop dequeues while `max_concurrency` allows multiple tasks
- **THEN** each `run_projects` row in `state='queued'` is transitioned to `running` by at most one claim and is not processed by two concurrent `process_run_project` tasks

#### Scenario: Weekly batch still disables session persistence

- **WHEN** a weekly batch subprocess is built for Claude
- **THEN** the command includes `--no-session-persistence`

#### Scenario: MR scan subprocess omits session persistence disable flag

- **WHEN** a per-MR MR scan subprocess is built for Claude
- **THEN** the command does not include `--no-session-persistence`

#### Scenario: Triage runs before any MR agent subprocess

- **WHEN** an MR scan run project begins execution
- **THEN** `triage-mrs.py` completes and writes `eligible_mrs.json` before the first agent subprocess is spawned

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

---
### Requirement: Cancelled is a terminal run state

The backend SHALL treat `cancelled` as a terminal value for both `runs.status` and `run_projects.state`. A run or project in state `cancelled` MUST NOT be claimed, resumed, or re-executed.

#### Scenario: Cancelled run is not claimed for execution

- **WHEN** the run worker looks for queued work and a run's status is `cancelled`
- **THEN** no project belonging to that run is claimed


<!-- @trace
source: cancel-run
updated: 2026-07-22
code:
  - frontend/src/lib/format.ts
  - backend/src/runs.rs
  - backend/src/worker.rs
  - frontend/src/api.ts
  - backend/src/server.rs
  - frontend/src/pages/RunsPage.tsx
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/run_cancellation.rs
  - frontend/src/pages/RunsPage.test.tsx
-->

---
### Requirement: Run finalization preserves cancelled status

When the backend evaluates whether a run is complete, it MUST leave the run's status unchanged if that status is already `cancelled`. Projects that were still executing when the run was cancelled MUST NOT cause the run to be finalized as `success`, `partial`, or `failed`.

#### Scenario: Late-finishing project does not overwrite cancelled status

- **GIVEN** a run has status `cancelled` while one of its projects is still finishing
- **WHEN** that project completes and run finalization is evaluated
- **THEN** the run's status remains `cancelled`

##### Example: finalization outcomes by prior status

| run status before finalization | project outcomes | run status after |
| ------------------------------ | ---------------- | ---------------- |
| running | all succeeded | success |
| running | some skipped_timeout | partial |
| running | some failed, none skipped | failed |
| cancelled | any | cancelled |

<!-- @trace
source: cancel-run
updated: 2026-07-22
code:
  - frontend/src/lib/format.ts
  - backend/src/runs.rs
  - backend/src/worker.rs
  - frontend/src/api.ts
  - backend/src/server.rs
  - frontend/src/pages/RunsPage.tsx
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/run_cancellation.rs
  - frontend/src/pages/RunsPage.test.tsx
-->

---
### Requirement: Manual single-person run enqueues one project scoped to one person

The backend SHALL expose `POST /api/runs` accepting JSON `{ "trigger": "manual_person", "project_name": <string>, "person_id": <integer> }`.

The handler MUST reject the request with HTTP 400 when `project_name` is missing or empty, or when `person_id` is missing.

The handler MUST resolve `project_name` to a `projects` row and return HTTP 404 when no such project exists. The handler MUST resolve `person_id` to a `people` row and return HTTP 404 when no such person exists. The handler MUST NOT reject the request based on whether the person has any commits, identities, or activity in the project.

When validation passes, the handler MUST create a `runs` row with `trigger='manual_person'`, `status='running'`, and `project_total=1`, and insert exactly one `run_projects` row for the resolved project with `state='queued'` and `person_id` set to the requested person.

The concurrency gate MUST be the same whole-system gate used by `manual_all` and `manual_project`: if any `run_projects` row is in state `queued` or `running` under an active run, the server MUST reject the new run with HTTP 409.

The response on success MUST be HTTP 201 with body `{ "run_id": <i64> }`.

#### Scenario: Start manual single-person run

- **WHEN** a client posts `{ "trigger": "manual_person", "project_name": "crm", "person_id": 1 }` and no run is active
- **THEN** the response is HTTP 201 with a run id, one `run_projects` row exists for the crm project with `state='queued'` and `person_id=1`

#### Scenario: Missing person_id is rejected

- **WHEN** a client posts `{ "trigger": "manual_person", "project_name": "crm" }` without `person_id`
- **THEN** the server responds HTTP 400 and creates no run

#### Scenario: Unknown person is rejected

- **WHEN** a client posts `manual_person` for an existing project but a `person_id` with no matching `people` row
- **THEN** the server responds HTTP 404 and creates no run

#### Scenario: Concurrency gate blocks single-person run

- **WHEN** any project already has a `run_projects` row in `queued` or `running` under an active run
- **AND** a client posts a `manual_person` run
- **THEN** the server responds HTTP 409 and creates no run


<!-- @trace
source: manual-person-rerun
updated: 2026-07-22
code:
  - backend/src/worker.rs
  - backend/src/runs.rs
  - backend/src/executor.rs
  - README.md
  - frontend/src/api.ts
  - frontend/src/lib/format.ts
  - backend/src/server.rs
  - frontend/src/pages/PeoplePage.tsx
  - backend/migrations/015_run_projects_person.sql
tests:
  - frontend/src/pages/PeoplePage.rerun.test.tsx
  - backend/tests/mr_reviews.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/executor_cancellation.rs
  - backend/tests/run_cancellation.rs
-->

---
### Requirement: run_projects carries an optional person scope

The `run_projects` table SHALL include a nullable `person_id` column referencing `people(id)`. A NULL `person_id` MUST mean the run project processes all resolved authors (the existing batch behavior for `manual_all`, `manual_project`, MR, scheduled, and poll runs). A non-NULL `person_id` MUST mean the run project is scoped to that single person.

When the worker claims a queued `run_projects` row, the claimed row data MUST include its `person_id` so downstream weekly manifest generation can honor the scope.

#### Scenario: Batch run projects have null person scope

- **WHEN** a `manual_all` or `manual_project` run is created
- **THEN** every inserted `run_projects` row has `person_id` NULL

#### Scenario: Claimed run project exposes person scope

- **WHEN** the worker claims a queued `run_projects` row created by a `manual_person` run for person 1
- **THEN** the claimed row data reports `person_id = 1`


<!-- @trace
source: manual-person-rerun
updated: 2026-07-22
code:
  - backend/src/worker.rs
  - backend/src/runs.rs
  - backend/src/executor.rs
  - README.md
  - frontend/src/api.ts
  - frontend/src/lib/format.ts
  - backend/src/server.rs
  - frontend/src/pages/PeoplePage.tsx
  - backend/migrations/015_run_projects_person.sql
tests:
  - frontend/src/pages/PeoplePage.rerun.test.tsx
  - backend/tests/mr_reviews.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/executor_cancellation.rs
  - backend/tests/run_cancellation.rs
-->

---
### Requirement: Weekly manifest is filtered to the run project person scope

When a weekly batch `run_projects` row has a non-NULL `person_id`, the backend SHALL filter the generated `manifest.json` so that `authors`, `open_pending`, and `published_pending_snippets` each contain only entries belonging to that person. Authors and open pending entries MUST be filtered by matching `person_id`. Published pending snippet paths MUST be filtered to those whose leading path segment equals that person's `people.display_name`.

When `person_id` is NULL, the manifest MUST include all resolved authors, all project open pending items, and all published pending snippets, unchanged from the existing batch behavior.

A person scope that matches no authors in the analysis window MUST NOT be an error: the manifest MUST be written with an empty `authors` array and the run MUST complete normally.

#### Scenario: Person-scoped manifest lists only the target person

- **GIVEN** a project with commits, open pending items, and published snippets for both person 1 ("Alice Chen") and person 2 ("Bob")
- **WHEN** the weekly manifest is written for a `run_projects` row with `person_id=1`
- **THEN** `authors` contains only Alice's entry, `open_pending` contains only Alice's items, and `published_pending_snippets` contains only paths under `Alice Chen/`

#### Scenario: Null person scope preserves full manifest

- **WHEN** the weekly manifest is written for a `run_projects` row with NULL `person_id`
- **THEN** `authors`, `open_pending`, and `published_pending_snippets` include every resolved author, open item, and published snippet for the project

#### Scenario: Person scope with no window activity completes as no-op

- **WHEN** the weekly manifest is written for a `person_id` whose person has no commits in the analysis window
- **THEN** the manifest is written with an empty `authors` array and the run project completes without error

<!-- @trace
source: manual-person-rerun
updated: 2026-07-22
code:
  - backend/src/worker.rs
  - backend/src/runs.rs
  - backend/src/executor.rs
  - README.md
  - frontend/src/api.ts
  - frontend/src/lib/format.ts
  - backend/src/server.rs
  - frontend/src/pages/PeoplePage.tsx
  - backend/migrations/015_run_projects_person.sql
tests:
  - frontend/src/pages/PeoplePage.rerun.test.tsx
  - backend/tests/mr_reviews.rs
  - backend/tests/runs_execution.rs
  - backend/tests/identity.rs
  - backend/tests/executor_cancellation.rs
  - backend/tests/run_cancellation.rs
-->