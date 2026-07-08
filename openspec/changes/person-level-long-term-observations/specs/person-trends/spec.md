## ADDED Requirements

### Requirement: Person-level report directory layout

The system SHALL store cross-project long-term observation files under `{DATA_ROOT_DIR}/reports/_people/{display_name}/`, where `display_name` MUST match the canonical `people.display_name` for that person.

The person-level directory MAY contain:

- `index.md` — cross-project long-term observation narrative
- `{YYYY-MM}.md` — monthly growth trajectory material
- `_notes.md` — historical pending questions in `- [YYYY-MM] question` line format

Project-level weekly reports SHALL remain under `{DATA_ROOT_DIR}/reports/{project_name}/{display_name}/{report_date}/` unchanged.

#### Scenario: Person directory is separate from project directories

- **WHEN** person "Alice Chen" participates in projects `game-backend` and `web-portal`
- **THEN** long-term observation for Alice lives at `reports/_people/Alice Chen/index.md`
- **AND** weekly summaries remain at `reports/game-backend/Alice Chen/{date}/summary.md` and `reports/web-portal/Alice Chen/{date}/summary.md`

#### Scenario: Underscore prefix avoids project name collision

- **WHEN** the backend scans `reports/` for project report roots
- **THEN** the `_people` directory MUST NOT be treated as a project name

---

### Requirement: Person trends read API

The backend SHALL expose `GET /api/people/:id/trends` returning JSON with fields:

- `person_id` (integer)
- `display_name` (string)
- `long_term_observation` (string) — full text of `_people/{display_name}/index.md`, or empty string if missing
- `growth_timeline` (array of objects with `month` and `content` strings) — derived from `_people/{display_name}/{YYYY-MM}.md` files sorted by month descending
- `historical_pending` (array of strings) — lines from `_notes.md` that start with `- [`

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

---

### Requirement: Loose-format migration support for person observations

The person trends reader MUST accept `index.md` files without YAML frontmatter or `summary.md` output-contract structure.

Administrators MAY place legacy free-form markdown directly into `_people/{display_name}/index.md` for display via the trends API without running weekly batch ingestion.

#### Scenario: Legacy markdown displays without frontmatter

- **GIVEN** `_people/Bob/index.md` contains plain markdown with no `---` frontmatter block
- **WHEN** a client calls `GET /api/people/:id/trends` for Bob
- **THEN** `long_term_observation` contains the full file text

---

### Requirement: Migration documentation for person observations

The repository MUST include `docs/idea/migration-person-observations.md` describing:

- The `_people/{display_name}/` directory layout
- That legacy notes may be pasted as free-form `index.md` without `summary.md` conversion
- That weekly `summary.md` ingestion rules remain unchanged for project-level reports

#### Scenario: Migration doc references person-level path

- **WHEN** a reader opens `docs/idea/migration-person-observations.md`
- **THEN** the document describes `reports/_people/{display_name}/index.md` as the cross-project observation location
