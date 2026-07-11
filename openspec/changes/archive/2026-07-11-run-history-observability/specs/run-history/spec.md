## ADDED Requirements

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

