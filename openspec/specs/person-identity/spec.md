# person-identity Specification

## Purpose

TBD - created by archiving change 'person-identity-resolution'. Update Purpose after archive.

## Requirements

### Requirement: Git author email is normalized for identity lookup

The backend SHALL normalize git author emails by trimming whitespace and converting to lowercase before using them as `person_identities.value` for `kind='git_email'`.

#### Scenario: Mixed-case email matches stored identity

- **WHEN** a commit author email is `Alice@Company.COM` and `person_identities` contains `('git_email', 'alice@company.com')`
- **THEN** the resolver returns the bound `person_id`


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
### Requirement: Unmatched authors are recorded during run preparation

Before starting the reviewer-batch subprocess for a project, the backend SHALL enumerate unique author emails with non-merge commits in the analysis window. For each email that does not match a `person_identities` row, the backend MUST upsert an `unmatched_authors` row with `kind='git_email'`, the normalized email as `value`, the project id, and an incremented `commit_count`.

#### Scenario: Unknown email creates unmatched author

- **WHEN** a project run is prepared and commit author `bob@gmail.com` has no matching identity
- **THEN** `unmatched_authors` contains a row for `('git_email', 'bob@gmail.com')` with `commit_count` greater than zero

#### Scenario: Known email does not create unmatched author

- **WHEN** commit author email matches an existing `person_identities` row
- **THEN** no new `unmatched_authors` row is created for that email


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
### Requirement: Unmatched authors list API

The backend SHALL expose `GET /api/unmatched-authors` returning an array of objects with fields `id`, `kind`, `value`, `project_id`, `project_name`, `commit_count`, `first_seen`, and `last_seen`.

#### Scenario: List unmatched authors

- **WHEN** two unmatched git emails exist across projects
- **THEN** the API response contains two entries with their project names

##### Example: unmatched list across projects

- **GIVEN** `unmatched_authors` has `bob@gmail.com` on project `game-backend` and `bob@personal.com` on project `web-portal`
- **WHEN** a client calls `GET /api/unmatched-authors`
- **THEN** the response contains two rows with `project_name` `game-backend` and `web-portal` respectively


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
### Requirement: Create person API

The backend SHALL expose `POST /api/people` accepting JSON `{ "display_name": "<string>" }` and returning `{ "id": <number>, "display_name": "<string>" }`.

The `display_name` MUST be unique among `people` rows. Duplicate names MUST be rejected with HTTP 409.

On creation the backend MUST set `folder_name` equal to the trimmed initial `display_name`. `folder_name` MUST be immutable thereafter.

#### Scenario: Create a new person

- **WHEN** a client posts `{ "display_name": "Alice Chen" }` and no person with that name exists
- **THEN** the response status is 201 and a new `people` row exists

#### Scenario: New person folder_name equals initial display_name

- **WHEN** a client posts `{ "display_name": "Alice Chen" }`
- **THEN** the created row has `folder_name` equal to "Alice Chen"


<!-- @trace
source: people-folder-name
updated: 2026-07-23
code:
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/lib/format.ts
  - backend/src/executor.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/migrations/016_people_folder_name.sql
  - backend/src/identity.rs
  - README.md
  - backend/src/runs.rs
  - skills/reviewer-batch/output-contract.md
  - backend/src/worker.rs
  - backend/src/pending_items.rs
  - backend/src/server.rs
  - backend/src/report_chat.rs
  - backend/src/error.rs
  - backend/migrations/015_run_projects_person.sql
  - frontend/src/api.ts
  - backend/src/mr_reviews.rs
  - backend/src/person_trends.rs
  - backend/src/summary.rs
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/runs_execution.rs
  - frontend/src/pages/PeoplePage.rerun.test.tsx
  - backend/tests/identity.rs
  - backend/tests/mr_reviews.rs
  - backend/tests/run_cancellation.rs
-->

---
### Requirement: Bind identity to person API

The backend SHALL expose `POST /api/people/:id/identities` accepting JSON `{ "kind": "<string>", "value": "<string>", "label": "<string|null>" }`.

Supported kinds for the people-settings UI MUST include `git_email`, `gitlab_user`, and `glab_user`. The backend MUST continue to normalize `git_email` values by trimming and lowercasing. For `gitlab_user` and `glab_user`, the backend MUST trim whitespace and MUST NOT force lowercase.

On success, the backend MUST insert a `person_identities` row and remove any matching `unmatched_authors` row with the same `(kind, value)`.

If `(kind, value)` is already bound to a different `person_id`, the server MUST respond with HTTP 409.

If `(kind, value)` is already bound to the same `person_id`, the server MUST treat the request as a no-op success without inserting a duplicate row.

#### Scenario: Bind email and clear unmatched queue

- **WHEN** `unmatched_authors` contains `('git_email', 'alice@co.com')` and a client binds that email to person id 1
- **THEN** `person_identities` contains the binding and `unmatched_authors` no longer contains that email

#### Scenario: Reject duplicate identity binding

- **WHEN** `('git_email', 'alice@co.com')` is already bound to person id 1
- **THEN** binding the same email to person id 2 returns HTTP 409

#### Scenario: Bind gitlab_user identity

- **WHEN** a client binds `{ "kind": "gitlab_user", "value": "alice.chen" }` to person id 1
- **THEN** `person_identities` contains that row for person id 1

#### Scenario: Same-person rebind is no-op

- **GIVEN** person id 1 already has `('git_email', 'alice@co.com')`
- **WHEN** a client binds the same kind and value to person id 1 again
- **THEN** the response indicates success
- **AND** only one matching `person_identities` row exists


<!-- @trace
source: people-settings-ui
updated: 2026-07-11
code:
  - .kiro/prompts/spectra-commit.prompt.md
  - backend/migrations/010_pending_items_indexes.sql
  - .spectra.yaml
  - .kiro/skills/spectra-discuss/SKILL.md
  - docs/idea/schema.md
  - .kiro/skills/spectra-commit/SKILL.md
  - backend/src/dashboard.rs
  - frontend/src/style.css
  - .kiro/skills/spectra-drift/SKILL.md
  - backend/src/error.rs
  - backend/src/lib.rs
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/prompts/spectra-ingest.prompt.md
  - frontend/src/api.ts
  - backend/src/reports.rs
  - .kiro/skills/spectra-apply/SKILL.md
  - .kiro/prompts/spectra-debug.prompt.md
  - backend/src/pending_items.rs
  - .kiro/prompts/spectra-propose.prompt.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-archive.prompt.md
  - README.md
  - .kiro/skills/spectra-propose/SKILL.md
  - backend/src/server.rs
  - frontend/src/types.ts
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-ingest/SKILL.md
  - .kiro/prompts/spectra-discuss.prompt.md
  - backend/src/identity.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - backend/src/summary.rs
  - docs/idea/roadmap-workflow-growth.md
  - frontend/src/app.ts
  - .kiro/prompts/spectra-audit.prompt.md
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/person_trends.rs
  - .kiro/prompts/spectra-ask.prompt.md
tests:
  - backend/tests/person_trends.rs
  - backend/tests/identity.rs
  - backend/tests/pending_items.rs
  - backend/tests/report_reader.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: List identities for a person API

The backend SHALL expose `GET /api/people/:id/identities` returning an array of objects with fields `id`, `kind`, `value`, and `label`.

#### Scenario: List bound identities

- **WHEN** person id 1 has two identity rows
- **THEN** the API returns both identities


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
### Requirement: Administrator can pre-register identities before review

The system SHALL allow creating a person and binding one or more `git_email` identities before any review run. When a subsequent run encounters commits from those emails, the backend MUST resolve them to the pre-registered `person_id` without creating an `unmatched_authors` row.

#### Scenario: Pre-bound email skips unmatched queue

- **WHEN** person "Alice Chen" is created and `alice@company.com` is bound before the first run
- **THEN** run preparation resolves Alice's commits to that person and does not insert `unmatched_authors` for that email


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
### Requirement: Frontend exposes unmatched author management

The frontend SHALL display the count of unmatched authors on the People Settings navigation item when the count is greater than zero, and SHALL provide an unmatched-authors section inside the People Settings view to bind each unmatched author to an existing person or create a new person and bind in one action.

The frontend MUST NOT require a separate global header unmatched-authors panel as the primary entry point.

#### Scenario: Bind unmatched author from UI

- **WHEN** the user selects an unmatched author in People Settings and chooses an existing person to bind
- **THEN** the unmatched count decreases and the binding succeeds without a full page reload

##### Example: bind from people-settings unmatched section

- **GIVEN** unmatched author `alice@gmail.com` on project `game-backend` and existing person id 1 "Alice Chen"
- **WHEN** the user binds that unmatched email to person id 1 from People Settings
- **THEN** `GET /api/unmatched-authors` no longer lists `alice@gmail.com` and person id 1 has `identity_count` increased by 1

#### Scenario: Nav badge reflects unmatched count

- **WHEN** at least one unmatched author exists
- **THEN** the People Settings sidebar item shows a count badge
- **AND** when the unmatched list becomes empty after binding, the badge is hidden


<!-- @trace
source: frontend-react-redesign
updated: 2026-07-12
code:
  - docs/design_handoff_reviewer_redesign/support.js
  - frontend/vite.config.ts
  - frontend/tsconfig.json
  - frontend/src/components/ui/Tabs.tsx
  - frontend/src/lib/catchup.ts
  - frontend/src/components/ui/Card.tsx
  - frontend/src/components/layout/Banner.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/lib/format.ts
  - frontend/src/App.tsx
  - frontend/src/components/layout/Sidebar.tsx
  - frontend/src/components/ui/Input.tsx
  - frontend/src/app.ts
  - frontend/src/components/ui/index.ts
  - frontend/src/lib/icons.ts
  - frontend/src/main.tsx
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/package.json
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - frontend/index.html
  - frontend/src/components/ui/Button.tsx
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/components/ui/NavItem.tsx
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Badge.tsx
  - frontend/src/style.css
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/index.css
  - frontend/src/pages/RunsPage.tsx
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/components/ui/Avatar.tsx
  - frontend/src/components/ui/StatCard.tsx
  - frontend/src/lib/tokens.ts
  - frontend/src/context/BannerContext.tsx
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/pages/ProjectsPage.tsx
  - frontend/src/main.ts
  - frontend/src/pages/DashboardPage.tsx
  - frontend/src/hooks/useApi.ts
tests:
  - frontend/src/hooks/useApi.test.ts
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - frontend/src/test/setup.ts
  - frontend/src/components/ui/atoms.test.tsx
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/components/layout/Banner.test.tsx
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - frontend/src/lib/catchup.test.ts
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/theme.test.ts
  - frontend/src/App.routes.test.tsx
-->

---
### Requirement: Unbind identity from person API

The backend SHALL expose `DELETE /api/people/{id}/identities/{identity_id}` that removes the `person_identities` row when it belongs to the given person.

On success the response status MUST be 204.

If the person does not exist, the identity does not exist, or the identity belongs to a different person, the response status MUST be 404.

Deleting the person's last remaining identity MUST be allowed.

#### Scenario: Delete an identity

- **GIVEN** person id 1 has identity id 9
- **WHEN** a client sends `DELETE /api/people/1/identities/9`
- **THEN** the response status is 204
- **AND** `GET /api/people/1/identities` no longer includes identity id 9

#### Scenario: Delete identity for wrong person returns 404

- **GIVEN** identity id 9 belongs to person id 1
- **WHEN** a client sends `DELETE /api/people/2/identities/9`
- **THEN** the response status is 404
- **AND** identity id 9 still exists

#### Scenario: Deleting the last identity is allowed

- **GIVEN** person id 1 has exactly one identity
- **WHEN** that identity is deleted
- **THEN** the response status is 204
- **AND** `GET /api/people/1/identities` returns an empty array

<!-- @trace
source: people-settings-ui
updated: 2026-07-11
code:
  - .kiro/prompts/spectra-commit.prompt.md
  - backend/migrations/010_pending_items_indexes.sql
  - .spectra.yaml
  - .kiro/skills/spectra-discuss/SKILL.md
  - docs/idea/schema.md
  - .kiro/skills/spectra-commit/SKILL.md
  - backend/src/dashboard.rs
  - frontend/src/style.css
  - .kiro/skills/spectra-drift/SKILL.md
  - backend/src/error.rs
  - backend/src/lib.rs
  - .kiro/skills/spectra-audit/SKILL.md
  - .kiro/prompts/spectra-ingest.prompt.md
  - frontend/src/api.ts
  - backend/src/reports.rs
  - .kiro/skills/spectra-apply/SKILL.md
  - .kiro/prompts/spectra-debug.prompt.md
  - backend/src/pending_items.rs
  - .kiro/prompts/spectra-propose.prompt.md
  - .kiro/skills/spectra-archive/SKILL.md
  - .kiro/prompts/spectra-archive.prompt.md
  - README.md
  - .kiro/skills/spectra-propose/SKILL.md
  - backend/src/server.rs
  - frontend/src/types.ts
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-ingest/SKILL.md
  - .kiro/prompts/spectra-discuss.prompt.md
  - backend/src/identity.rs
  - .kiro/skills/spectra-debug/SKILL.md
  - backend/src/summary.rs
  - docs/idea/roadmap-workflow-growth.md
  - frontend/src/app.ts
  - .kiro/prompts/spectra-audit.prompt.md
  - .kiro/prompts/spectra-apply.prompt.md
  - .kiro/skills/spectra-ask/SKILL.md
  - backend/src/person_trends.rs
  - .kiro/prompts/spectra-ask.prompt.md
tests:
  - backend/tests/person_trends.rs
  - backend/tests/identity.rs
  - backend/tests/pending_items.rs
  - backend/tests/report_reader.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: People have an immutable folder_name path key

The `people` table SHALL include a `folder_name` column that is NOT NULL and UNIQUE. `folder_name` MUST be set once when a person is created and MUST equal the person's initial `display_name` (trimmed). No API MUST change `folder_name` after creation.

`folder_name` SHALL be the single stable key used for all on-disk report paths and for resolving a person from summary output. `display_name` MUST NOT be used as an on-disk path segment or as the ingest resolution key.

For rows that exist before this capability is introduced, `folder_name` MUST be backfilled from the current `display_name`.

#### Scenario: Existing rows backfill folder_name from display_name

- **GIVEN** a `people` row with `display_name` "Alice Chen" created before this capability
- **WHEN** the folder_name column is introduced
- **THEN** that row's `folder_name` is "Alice Chen"

#### Scenario: folder_name is not mutated by any API

- **GIVEN** person with `folder_name` "Alice"
- **WHEN** any people API (create, rename, bind identity) is invoked for that person
- **THEN** `folder_name` remains "Alice"

<!-- @trace
source: people-folder-name
updated: 2026-07-23
code:
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/lib/format.ts
  - backend/src/executor.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/migrations/016_people_folder_name.sql
  - backend/src/identity.rs
  - README.md
  - backend/src/runs.rs
  - skills/reviewer-batch/output-contract.md
  - backend/src/worker.rs
  - backend/src/pending_items.rs
  - backend/src/server.rs
  - backend/src/report_chat.rs
  - backend/src/error.rs
  - backend/migrations/015_run_projects_person.sql
  - frontend/src/api.ts
  - backend/src/mr_reviews.rs
  - backend/src/person_trends.rs
  - backend/src/summary.rs
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/runs_execution.rs
  - frontend/src/pages/PeoplePage.rerun.test.tsx
  - backend/tests/identity.rs
  - backend/tests/mr_reviews.rs
  - backend/tests/run_cancellation.rs
-->