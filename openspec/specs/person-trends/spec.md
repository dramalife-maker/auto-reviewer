# person-trends Specification

## Purpose

TBD - created by archiving change 'person-level-long-term-observations'. Update Purpose after archive.

## Requirements

### Requirement: Person-level report directory layout

The system SHALL store cross-project long-term observation files under `{DATA_ROOT_DIR}/reports/_people/{display_name}/`, where `display_name` MUST match the canonical `people.display_name` for that person.

The person-level directory MAY contain:

- `index.md` — cross-project long-term observation narrative
- `{YYYY-MM}.md` — monthly growth trajectory material
- `_notes.md` — historical pending questions using:
  - open lines: `- [YYYY-MM] {question}`
  - resolved lines: `- [YYYY-MM→YYYY-MM] ✓ {question}` with optional trailing ` — {resolution_note}`

Project-level weekly reports SHALL remain under `{DATA_ROOT_DIR}/reports/{project_name}/{display_name}/{report_date}/` unchanged.

#### Scenario: Person directory is separate from project directories

- **WHEN** person "Alice Chen" participates in projects `game-backend` and `web-portal`
- **THEN** long-term observation for Alice lives at `reports/_people/Alice Chen/index.md`
- **AND** weekly summaries remain at `reports/game-backend/Alice Chen/{date}/summary.md` and `reports/web-portal/Alice Chen/{date}/summary.md`

#### Scenario: Underscore prefix avoids project name collision

- **WHEN** the backend scans `reports/` for project report roots
- **THEN** the `_people` directory MUST NOT be treated as a project name

#### Scenario: Notes file accepts open and resolved line forms

- **GIVEN** `_people/Alice Chen/_notes.md` contains both `- [2026-07] Why choose A?` and `- [2026-06→2026-07] ✓ Earlier concern`
- **WHEN** the trends reader loads historical pending for Alice
- **THEN** both lines are accepted as historical pending entries


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
### Requirement: Person trends read API

The backend SHALL expose `GET /api/people/:id/trends` returning JSON with fields:

- `person_id` (integer)
- `display_name` (string)
- `long_term_observation` (string) — full text of `_people/{display_name}/index.md`, or empty string if missing
- `growth_timeline` (array of objects with `month` and `content` strings) — derived from `_people/{display_name}/{YYYY-MM}.md` files sorted by month descending
- `historical_pending` (array of objects) — parsed from `_notes.md` lines that start with `- [`, each object MUST include:
  - `question` (string)
  - `status` (`open` or `resolved`)
  - `raised_month` (string, `YYYY-MM`)
  - `resolved_month` (string or null)
  - `resolution_note` (string or null)
  - `raw_line` (string)

A line matching `- [YYYY-MM→YYYY-MM] ✓ ...` MUST be parsed as `status=resolved`. A line matching `- [YYYY-MM] ...` without an arrow MUST be parsed as `status=open`.

The endpoint MUST resolve `display_name` from `people` by `person_id`. Unknown `person_id` MUST return HTTP 404.

Missing person-level files MUST NOT cause HTTP errors; the corresponding response fields MUST be empty.

#### Scenario: Trends API returns person-level index content

- **GIVEN** `reports/_people/Alice Chen/index.md` exists with markdown body
- **WHEN** a client calls `GET /api/people/:id/trends` for Alice's person id
- **THEN** `long_term_observation` contains the file contents
- **AND** the response status is 200

#### Scenario: Missing person-level files return empty sections

- **GIVEN** person id exists but `_people/{display_name}/` directory does not exist
- **WHEN** a client calls `GET /api/people/:id/trends`
- **THEN** `long_term_observation`, `growth_timeline`, and `historical_pending` are empty
- **AND** the response status is 200

#### Scenario: Historical pending distinguishes open and resolved lines

- **GIVEN** `_notes.md` contains `- [2026-07] Why choose A?` and `- [2026-06→2026-07] ✓ Earlier concern — fixed in review`
- **WHEN** a client calls `GET /api/people/:id/trends`
- **THEN** `historical_pending` contains one object with `status` `open` and `question` `Why choose A?`
- **AND** one object with `status` `resolved`, `resolved_month` `2026-07`, and `resolution_note` `fixed in review`

##### Example: notes line parsing

| raw_line | status | raised_month | resolved_month | question | resolution_note |
| --- | --- | --- | --- | --- | --- |
| `- [2026-07] Why choose A?` | `open` | `2026-07` | null | `Why choose A?` | null |
| `- [2026-06→2026-07] ✓ Earlier concern — fixed in review` | `resolved` | `2026-06` | `2026-07` | `Earlier concern` | `fixed in review` |


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
### Requirement: Loose-format migration support for person observations

The person trends reader MUST accept `index.md` files without YAML frontmatter or `summary.md` output-contract structure.

Administrators MAY place legacy free-form markdown directly into `_people/{display_name}/index.md` for display via the trends API without running weekly batch ingestion.

#### Scenario: Legacy markdown displays without frontmatter

- **GIVEN** `_people/Bob/index.md` contains plain markdown with no `---` frontmatter block
- **WHEN** a client calls `GET /api/people/:id/trends` for Bob
- **THEN** `long_term_observation` contains the full file text


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
### Requirement: Migration documentation for person observations

The repository MUST include `docs/idea/migration-person-observations.md` describing:

- The `_people/{display_name}/` directory layout
- That legacy notes may be pasted as free-form `index.md` without `summary.md` conversion
- That weekly `summary.md` ingestion rules remain unchanged for project-level reports

#### Scenario: Migration doc references person-level path

- **WHEN** a reader opens `docs/idea/migration-person-observations.md`
- **THEN** the document describes `reports/_people/{display_name}/index.md` as the cross-project observation location

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
### Requirement: Project .notes directory is reserved metadata

Under `{DATA_ROOT_DIR}/reports/{project_name}/`, the directory named `.notes` is reserved for project-level ADR storage as defined by the `project-adr-notes` capability.

Any logic that treats immediate children of `reports/{project_name}/` as engineer folders keyed by `display_name` MUST skip `.notes`.

This requirement does not change the person-level layout under `reports/_people/`, which remains the home of `index.md`, monthly growth files, and `_notes.md` pending history.

#### Scenario: Trends and report scans ignore .notes as a person

- **WHEN** code or a workflow enumerates person directories beneath `reports/game-backend/`
- **THEN** `.notes` is skipped and is not loaded as person trends or weekly person roots

#### Scenario: People notes remain under _people

- **WHEN** a pending item is resolved for Alice
- **THEN** the historical pending line is still written to `reports/_people/Alice Chen/_notes.md` and NOT to `reports/{project}/.notes/`

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