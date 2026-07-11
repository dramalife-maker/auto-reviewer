# report-reader Specification

## Purpose

TBD - created by archiving change 'cloud-reviewer-mvp'. Update Purpose after archive.

## Requirements

### Requirement: People list API exposes read and pending status

The backend SHALL expose `GET /api/people` returning an array of objects with fields `id`, `display_name`, `project_count`, `unread_count`, `open_pending_count`, and `identity_count` computed per `docs/idea/schema.md` query patterns.

#### Scenario: Unread badge reflects unread reports

- **WHEN** a person has at least one report with `is_read=0`
- **THEN** that person's `unread_count` in the API response is greater than zero

#### Scenario: Identity count reflects bound identities

- **WHEN** a person has two rows in `person_identities`
- **THEN** that person's `identity_count` in the API response is 2


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
### Requirement: Latest weekly report content is served per person

The backend SHALL expose `GET /api/people/:id/reports/latest` returning, for each project with a report on the person's latest `report_date`, JSON containing `project_name`, `one_line`, `mr_count`, `commit_count`, rendered sections `highlights` and `growth` parsed from `summary.md`, and `pending_items` loaded from SQLite.

Each `pending_items` element MUST include at least `id`, `question`, `status`, `raised_date`, `project_id`, and `project_name`, and MUST only include rows with `status='open'` for that person and project.

The response MUST NOT include a `pending` string array derived from `summary.md` `## 待確認` for UI rendering. Workflow output and ingestion of `## 待確認` into `pending_items` remain unchanged.

The weekly overview API behavior for highlights and growth MUST remain unchanged. Long-term cross-project observation MUST NOT be included in this endpoint; it is served only by the trends API.

#### Scenario: Latest reports excludes long-term observation

- **WHEN** a client calls `GET /api/people/:id/reports/latest`
- **THEN** the response contains per-project weekly cards only
- **AND** does not include person-level `index.md` content

#### Scenario: Latest reports pending comes from open DB rows

- **GIVEN** person Alice has an open `pending_items` row for project `game-backend` with question `Why choose A?`
- **AND** Alice's latest summary.md also lists that question under `## 待確認`
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** the `game-backend` card `pending_items` array contains an object with that question and a numeric `id`
- **AND** the card does not expose a `pending` string array field

#### Scenario: Resolved items are omitted from latest pending_items

- **GIVEN** person Alice has only a resolved `pending_items` row for project `game-backend`
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** the `game-backend` card `pending_items` array is empty


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
### Requirement: Reports can be marked read

The backend SHALL expose `PATCH /api/reports/:id/read` setting `reports.is_read=1` for the given id.

#### Scenario: Mark report read

- **WHEN** a client sends `PATCH /api/reports/42/read` for an existing unread report
- **THEN** subsequent `GET /api/people` shows decreased `unread_count` for that person


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
### Requirement: Web UI displays weekly reader and run controls

The frontend SHALL provide a page with a people sidebar, a main panel showing the selected person's latest weekly content, a control to trigger `POST /api/runs` with `manual_all`, a visible notification when the latest run transitions to terminal status `success`, `partial`, or `failed`, an unmatched-authors count indicator, and a panel to bind unmatched authors to existing people or create new people.

#### Scenario: User triggers batch run from UI

- **WHEN** the user clicks the run-all control
- **THEN** the client sends `POST /api/runs` and displays in-progress status until the run completes

#### Scenario: User marks content as read

- **WHEN** the user opens a person's report and activates mark-read
- **THEN** the client calls `PATCH /api/reports/:id/read` and the sidebar unread indicator clears for that report

#### Scenario: Unmatched count is visible before running review

- **WHEN** `GET /api/unmatched-authors` returns a non-empty list
- **THEN** the UI shows the unmatched author count without requiring a manual refresh after page load

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
### Requirement: Person trends API for report reader

The backend SHALL expose `GET /api/people/:id/trends` as defined in the `person-trends` capability. The report reader frontend MUST consume this endpoint when the user views the trends section for a selected person.

#### Scenario: Frontend fetches trends for selected person

- **WHEN** a user selects a person and opens the trends view
- **THEN** the client calls `GET /api/people/{id}/trends`
- **AND** renders `long_term_observation`, `growth_timeline`, and `historical_pending` sections

#### Scenario: Trends empty state

- **WHEN** trends API returns empty `long_term_observation` and empty arrays
- **THEN** the UI shows an empty-state message indicating no long-term data yet


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
