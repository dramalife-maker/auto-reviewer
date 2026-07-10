## MODIFIED Requirements

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

