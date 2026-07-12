# people-settings Specification

## Purpose

TBD - created by archiving change 'people-settings-ui'. Update Purpose after archive.

## Requirements

### Requirement: Person detail API includes identities and projects

The backend SHALL expose `GET /api/people/{id}` returning JSON with:

- `id` (integer)
- `display_name` (string)
- `identities` (array of `{ id, kind, value, label }`)
- `projects` (array of `{ id, name }` for distinct projects linked via `reports` or `participation` for that person)

Unknown `person_id` MUST return HTTP 404.

When the person has no reports and no participation rows, `projects` MUST be an empty array.

#### Scenario: Detail returns identities and projects

- **GIVEN** person id 1 has two identities and reports in projects `game-backend` and `web-portal`
- **WHEN** a client calls `GET /api/people/1`
- **THEN** the response includes both identities
- **AND** `projects` contains both project names

#### Scenario: Unknown person detail returns 404

- **WHEN** a client calls `GET /api/people/{id}` for a non-existent id
- **THEN** the response status is 404


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
### Requirement: Person display name can be renamed

The backend SHALL expose `PATCH /api/people/{id}` accepting JSON `{ "display_name": "<string>" }`.

The new `display_name` MUST be trimmed and non-empty. Empty names MUST return HTTP 400. Duplicate names belonging to another person MUST return HTTP 409.

On success the backend MUST update `people.display_name`. If `{DATA_ROOT_DIR}/reports/_people/{old_display_name}/` exists, the backend MUST rename that directory to `{DATA_ROOT_DIR}/reports/_people/{new_display_name}/`. If the destination directory already exists, the backend MUST return HTTP 409 and MUST NOT change the database row.

If the directory rename fails after a database update, the backend MUST roll back `people.display_name` to the previous value and return an error status.

The backend MUST NOT delete people via this change. The backend MUST NOT rename project-level report directories under `reports/{project_name}/{display_name}/`.

#### Scenario: Rename updates database and people directory

- **GIVEN** person "Alice" with `reports/_people/Alice/` present
- **WHEN** a client patches display_name to `Alice Chen`
- **THEN** `people.display_name` is `Alice Chen`
- **AND** the directory is renamed to `reports/_people/Alice Chen/`

#### Scenario: Rename rejects colliding destination directory

- **GIVEN** person "Alice" and an existing directory `reports/_people/Alice Chen/`
- **WHEN** a client patches Alice's display_name to `Alice Chen`
- **THEN** the response status is 409
- **AND** `people.display_name` remains `Alice`

#### Scenario: Rename rejects duplicate display name

- **GIVEN** people "Alice" and "Bob"
- **WHEN** a client patches Bob's display_name to `Alice`
- **THEN** the response status is 409


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
### Requirement: People settings UI manages persons and identities

The frontend SHALL provide a dedicated people-settings view (separate from the weekly report reader) with:

- a list of people and a control to create a new person
- an editor for the selected person's `display_name`
- identity list with add and remove controls supporting kinds `git_email`, `gitlab_user`, and `glab_user`
- a read-only list of participating project names rendered as a plain bullet list
- an unmatched-authors management section at the top of the people-settings view for binding unmatched authors to an existing person or creating a new person and binding in one action

The people-settings view MUST NOT offer a delete-person action.

The frontend MUST NOT require a global app-header unmatched-authors shortcut panel.

#### Scenario: Create and bind identity from settings view

- **WHEN** a manager creates person "Alice Chen" and binds `git_email` `alice@co.com` from the people-settings view
- **THEN** subsequent `GET /api/people/{id}` shows that identity
- **AND** unmatched authors are manageable from the people-settings unmatched section without using an app-header panel

#### Scenario: Remove identity from settings view

- **WHEN** a manager removes an identity from the selected person in people-settings
- **THEN** that identity no longer appears in `GET /api/people/{id}/identities`

#### Scenario: Bind unmatched author from people settings

- **WHEN** unmatched authors exist and the manager opens People Settings
- **THEN** the unmatched section lists those authors
- **AND** binding one to an existing person decreases the unmatched count without a full page reload

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