## MODIFIED Requirements

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

