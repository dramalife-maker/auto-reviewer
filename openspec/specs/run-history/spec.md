# run-history Specification

## Purpose

Managers can browse historical reviewer runs (list + detail), see per-project timing/errors, inspect MR skip reasons from `eligible_mrs.json`, and for finished runs see whether MR drafts or weekly reports were produced with navigation hints into the inbox and report reader. The dashboard exposes up to five recent runs as an entry point.

## Requirements

### Requirement: Runs history list API

The backend SHALL expose `GET /api/runs` returning a JSON object `{ "runs": [...], "total": <number> }` ordered by `started_at` descending.

Each run list item MUST include: `id`, `trigger`, `status`, `started_at`, `finished_at` (nullable), `duration_sec` (nullable), `project_total` (nullable), `project_skipped`.

Query parameters:

- `limit` — optional positive integer, default 50, maximum 200
- `offset` — optional non-negative integer, default 0
- `trigger` — optional exact match filter
- `status` — optional exact match filter

Invalid `limit` or `offset` MUST return HTTP 400.

#### Scenario: List returns newest first

- **GIVEN** three runs with distinct `started_at`
- **WHEN** a client calls `GET /api/runs`
- **THEN** `runs` are ordered by `started_at` descending
- **AND** `total` equals 3

#### Scenario: Limit and offset paginate

- **GIVEN** at least three runs
- **WHEN** a client calls `GET /api/runs?limit=1&offset=1`
- **THEN** exactly one run is returned
- **AND** `total` reflects the full matching count

#### Scenario: Filter by trigger

- **WHEN** a client calls `GET /api/runs?trigger=mr_poll`
- **THEN** every returned run has `trigger` equal to `mr_poll`


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
### Requirement: Run detail includes timing and MR skip summary

The backend SHALL continue to expose `GET /api/runs/{id}` and MUST include:

- run-level `duration_sec` and `note` (nullable)
- each project entry: `name`, `state`, `error`, `started_at`, `finished_at`, `duration_sec` (nullable fields as stored)

For runs whose `trigger` is `mr_poll` or `manual_mr_poll`, each project entry MUST also include `skip_summary` with:

- `by_reason`: object mapping skip reason string to count
- `items`: array of `{ "mr_iid": number, "skip_reason": string }` capped at 100 entries

The skip summary MUST be derived from that project's `eligible_mrs.json` `skipped` array under the run layout. Missing or unreadable files MUST yield an empty summary (`by_reason` empty object, `items` empty array) without failing the whole response.

Unknown run id MUST return HTTP 404.

Non-MR triggers MUST omit `skip_summary` or set it to null.

#### Scenario: Detail returns per-project error and duration

- **GIVEN** a finished run where project `game-backend` failed with an error message
- **WHEN** a client calls `GET /api/runs/{id}`
- **THEN** that project entry has `state` `failed`, non-null `error`, and `duration_sec` when stored

#### Scenario: MR run exposes skip summary from eligible file

- **GIVEN** an `mr_poll` run for project id 3 whose `eligible_mrs.json` skipped MR 12 for `inbox_draft` and MR 8 for `gitlab_draft`
- **WHEN** a client calls `GET /api/runs/{id}`
- **THEN** that project's `skip_summary.by_reason` counts both reasons
- **AND** `items` includes both MR IIDs

#### Scenario: Missing eligible file yields empty skip summary

- **GIVEN** an `mr_poll` run project with no `eligible_mrs.json`
- **WHEN** a client calls `GET /api/runs/{id}`
- **THEN** that project's `skip_summary.items` is empty
- **AND** the response status is 200

##### Example: skip_summary shape

| skipped rows | by_reason | items (truncated) |
| --- | --- | --- |
| `(12, inbox_draft)`, `(8, gitlab_draft)` | `{ "inbox_draft": 1, "gitlab_draft": 1 }` | both entries |
| (none) | `{}` | `[]` |


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
### Requirement: Execution history UI with dashboard entry

The frontend SHALL provide an execution-history view listing runs and a detail pane/page for a selected run.

The app navigation SHALL include an entry to this view. The dashboard SHALL show up to five recent runs (from dashboard payload or equivalent) with links into the history view/detail.

The detail view MUST show per-project states, errors, and for MR runs the skip summary grouped by reason.

#### Scenario: Open history from dashboard

- **GIVEN** dashboard `recent_runs` contains at least one run
- **WHEN** the manager opens the execution history from the dashboard
- **THEN** the runs list view is shown

#### Scenario: Inspect MR skips in detail

- **GIVEN** an MR poll run with non-empty `skip_summary`
- **WHEN** the manager opens that run's detail
- **THEN** skip reasons and MR IIDs are visible

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
### Requirement: Run detail includes project outputs summary

For runs whose `status` is not `running`, each project entry on `GET /api/runs/{id}` MUST include an `outputs` object (or omit/`null` when empty) that summarizes artifacts produced for that `(run_id, project_id)`:

- `mr_drafts`: `{ "count": <number> }` when the run layout drafts directory for that project contains one or more `*.md` files; otherwise omit or `null`
- `weekly_reports`: `{ "people": [ { "person_id": <number>, "display_name": <string> }, ... ] }` listing people with a `reports` row for that `run_id` and `project_id`; empty list MUST omit `weekly_reports` or set it to `null`

While the run `status` is `running`, every project entry MUST omit `outputs` or set it to `null`.

Missing or unreadable drafts directories MUST NOT fail the whole response; treat draft count as zero.

Project `state` (including `failed` / `skipped_timeout`) MUST NOT suppress `outputs` when artifacts exist.

#### Scenario: Finished MR run exposes draft count

- **WHEN** a finished run has at least one `*.md` under that project's run drafts directory
- **THEN** that project's `outputs.mr_drafts.count` equals the number of those markdown files

#### Scenario: Finished weekly run exposes people from reports

- **WHEN** a finished run has `reports` rows for the project with that `run_id`
- **THEN** that project's `outputs.weekly_reports.people` includes each person's `person_id` and `display_name`

#### Scenario: Running run omits outputs

- **WHEN** a client calls `GET /api/runs/{id}` for a run with `status` `running`
- **THEN** each project entry omits `outputs` or sets it to `null`

#### Scenario: Missing drafts directory yields no mr_drafts

- **WHEN** a finished MR run project has no drafts directory
- **THEN** `outputs.mr_drafts` is omitted or `null`
- **AND** the response status is 200

##### Example: outputs shape

| drafts `*.md` | reports people | outputs (truncated) |
| --- | --- | --- |
| 2 files | none | `{ "mr_drafts": { "count": 2 }, "weekly_reports": null }` |
| none | Alice (id 1) | `{ "mr_drafts": null, "weekly_reports": { "people": [{ "person_id": 1, "display_name": "Alice" }] } }` |
| none | none | `null` / omitted |

<!-- @trace
source: run-history-outputs-hints
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/server.rs
  - frontend/src/types.ts
tests:
  - backend/tests/runs_execution.rs
-->

---
### Requirement: Execution history detail shows outputs navigation hints

The run detail「專案結果」view MUST show, for each project with non-empty `outputs`:

- When `mr_drafts.count` is greater than zero: text stating that N MR drafts were produced, with a link to `/mr-inbox` labeled as the MR inbox
- When `weekly_reports.people` is non-empty: text stating weekly reports were produced for those people, with each of the first eight `display_name` values linking to `/reports/{person_id}`; if more than eight people, the UI MUST indicate the remaining count (for example「…等共 N 人」)

Hints MUST appear regardless of project `state` when `outputs` is present. Projects without `outputs` MUST NOT show an empty outputs section.

#### Scenario: MR draft hint links to inbox

- **WHEN** the manager opens a finished run whose project has `outputs.mr_drafts.count` = 2
- **THEN** the project card shows that 2 MR drafts were produced
- **AND** a link to `/mr-inbox` is available

#### Scenario: Weekly report hint links to people

- **WHEN** the manager opens a finished run whose project lists Alice and Bob under `outputs.weekly_reports.people`
- **THEN** the project card shows their names as links to `/reports/{person_id}`

#### Scenario: No outputs hides hints

- **WHEN** a project entry has no `outputs`
- **THEN** the project card does not render an outputs navigation hint block

<!-- @trace
source: run-history-outputs-hints
updated: 2026-07-14
code:
  - frontend/src/pages/RunsPage.tsx
  - frontend/src/types.ts
tests:
  - frontend/src/pages/RunsPage.test.tsx
-->
