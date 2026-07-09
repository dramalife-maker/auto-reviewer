## MODIFIED Requirements

### Requirement: People list API exposes read and pending status

The backend SHALL expose `GET /api/people` returning an array of objects with fields `id`, `display_name`, `project_count`, `unread_count`, `open_pending_count`, and `identity_count` computed per `docs/idea/schema.md` query patterns.

#### Scenario: Unread badge reflects unread reports

- **WHEN** a person has at least one report with `is_read=0`
- **THEN** that person's `unread_count` in the API response is greater than zero

#### Scenario: Identity count reflects bound identities

- **WHEN** a person has two rows in `person_identities`
- **THEN** that person's `identity_count` in the API response is 2

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

