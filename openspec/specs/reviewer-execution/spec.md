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