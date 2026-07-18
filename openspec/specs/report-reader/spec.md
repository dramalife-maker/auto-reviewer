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
### Requirement: Latest reports include pending MR observation snippets

The backend SHALL include a `pending_observations` array on each project card returned by `GET /api/people/:id/reports/latest`.

Each element MUST represent one file that still exists under `reports/<project_name>/<person_display_name>/_pending/` whose filename matches `mr-{mr_iid}-round-{review_round}.md`, and MUST include:

- `mr_iid` (number)
- `review_round` (number)
- `mr_title` (string or null; from the matching `mr_reviews` row when present)
- `status` — one of `draft`, `published`, `ignored`, or `unknown` (from the matching `mr_reviews` row; `unknown` when no row matches)
- `filename` (string)
- `content` (string; full file contents)

The array MUST be empty when the `_pending/` directory is missing or contains no parseable snippet files. Files that fail to parse or read MUST be omitted without failing the whole response. Snippets already consumed (file removed from `_pending/`) MUST NOT appear.

Ordering MUST be: `published`, then `draft`, then `ignored`, then `unknown`; within the same status, ascending `mr_iid` then ascending `review_round`.

This field MUST NOT replace or alter `pending_items` (SQLite open 待確認 rows).

#### Scenario: Draft and published snippets both appear

- **GIVEN** person Alice has a latest weekly report for project `game-backend`
- **AND** `reports/game-backend/Alice/_pending/mr-4-round-1.md` exists with corresponding `mr_reviews.status='draft'`
- **AND** `reports/game-backend/Alice/_pending/mr-7-round-1.md` exists with corresponding `mr_reviews.status='published'`
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** the `game-backend` card `pending_observations` contains both snippets
- **AND** the published snippet appears before the draft snippet
- **AND** each element exposes the correct `status` and non-empty `content`

#### Scenario: Consumed snippet is omitted

- **GIVEN** person Alice has a published `mr_reviews` row for MR 4 round 1
- **AND** the corresponding file is absent from `_pending/`
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** `pending_observations` does not include an entry for MR 4 round 1

#### Scenario: Orphan snippet is marked unknown

- **GIVEN** `reports/game-backend/Alice/_pending/mr-9-round-1.md` exists
- **AND** no `mr_reviews` row matches that project, MR, and round
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** that snippet appears with `status` equal to `unknown`

#### Scenario: Empty pending directory yields empty array

- **GIVEN** Alice has a latest report for `game-backend`
- **AND** `reports/game-backend/Alice/_pending/` is missing or empty
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** the `game-backend` card `pending_observations` is an empty array

#### Scenario: Pending observations without any weekly report

- **GIVEN** person Alice exists and has no rows in `reports`
- **AND** `reports/game-backend/Alice/_pending/mr-4-round-1.md` exists with corresponding `mr_reviews.status='draft'`
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** the response has `report_date` equal to null
- **AND** the response contains a `game-backend` project card whose `pending_observations` includes that snippet
- **AND** that card has empty `highlights` and `growth`

#### Scenario: Pending observations for a project without a latest-week report

- **GIVEN** person Alice has a latest weekly report only for project `alpha`
- **AND** `reports/beta/Alice/_pending/mr-2-round-1.md` exists
- **WHEN** a client calls `GET /api/people/:id/reports/latest` for Alice
- **THEN** the response includes both an `alpha` card (from the weekly report) and a `beta` card carrying that pending observation


<!-- @trace
source: report-reader-pending-observations
updated: 2026-07-16
code:
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/types.ts
  - backend/src/worker.rs
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: Report reader UI shows pending observation snippets

The report reader frontend SHALL render `pending_observations` from the latest-reports response.

- On the overview tab, when any project has a non-empty `pending_observations`, the UI MUST show a section titled for pending fold-in observations, grouped by project, displaying each snippet's status, MR identity (`mr_title` when present otherwise `mr_iid` / round), and full `content`.
- On a project tab, when that project's `pending_observations` is non-empty, the UI MUST show the same section for that project.
- When the response has project cards solely from pending observations (no weekly summary content), the UI MUST still render those cards and the pending-observations section, and MUST NOT show the empty-state message used when there are zero project cards.
- When all relevant arrays are empty, the UI MUST NOT show the section.
- The UI MUST keep the existing open `pending_items` (待確認) section separate and MUST NOT offer publish, ignore, or resolve actions on observation snippets from this page.

#### Scenario: Overview shows pending observations across projects

- **GIVEN** the latest-reports response includes a non-empty `pending_observations` on at least one project
- **WHEN** the user views the overview tab
- **THEN** the pending-observations section is visible with those snippets grouped by project

#### Scenario: Empty observations hide the section

- **GIVEN** every project card has `pending_observations` equal to `[]`
- **WHEN** the user views the overview or a project tab
- **THEN** the pending-observations section is not rendered

<!-- @trace
source: report-reader-pending-observations
updated: 2026-07-16
code:
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/types.ts
  - backend/src/worker.rs
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: Report reader hosts person Agent Chat

The report reader frontend SHALL show an Agent Chat panel for the selected person.

- The panel MUST load transcript via `GET /api/people/:id/report-chat` when a person is selected and MUST re-hydrate when the selected person changes.
- The panel MUST allow sending a message via `POST /api/people/:id/report-chat/agent-turn` and append the user message and assistant reply to the visible transcript on success.
- After a successful agent-turn, the panel MUST reload that person's latest reports so file edits made by the agent become visible.
- The panel MUST NOT offer publish or GitLab actions.
- When no person is selected, the Agent Chat panel MUST NOT be interactive for report chat.

#### Scenario: Selecting a person hydrates chat history

- **GIVEN** `GET /api/people/1/report-chat` returns two stored messages
- **WHEN** the operator opens the report reader for person 1
- **THEN** those messages are shown in the Agent Chat panel without re-sending

#### Scenario: Successful turn reloads reports

- **GIVEN** the operator is viewing person 1's reports
- **WHEN** an agent-turn succeeds
- **THEN** the client requests latest reports for person 1 again

<!-- @trace
source: report-reader-agent-chat
updated: 2026-07-17
code:
  - backend/src/server.rs
  - backend/src/worker.rs
  - backend/src/reports.rs
  - backend/src/summary.rs
  - frontend/src/types.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/executor.rs
  - backend/src/report_chat.rs
  - backend/src/lib.rs
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/api.ts
  - backend/src/mr_change_materials.rs
tests:
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->