## ADDED Requirements

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
