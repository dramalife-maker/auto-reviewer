## ADDED Requirements

### Requirement: People list API exposes read and pending status

The backend SHALL expose `GET /api/people` returning an array of objects with fields `id`, `display_name`, `project_count`, `unread_count`, and `open_pending_count` computed per `docs/idea/schema.md` query patterns.

#### Scenario: List includes unread badge data

- **WHEN** a person has at least one report with `is_read=0`
- **THEN** that person's `unread_count` in the API response is greater than zero

### Requirement: Latest weekly report content is served per person

The backend SHALL expose `GET /api/people/:id/reports/latest` returning, for each project with a report on the person's latest `report_date`, JSON containing `project_name`, `one_line`, `mr_count`, `commit_count`, rendered sections `highlights`, `growth`, and `pending` parsed from `summary.md`.

#### Scenario: Fetch latest cross-project summaries

- **WHEN** a person has reports for two projects on the same latest date
- **THEN** the response contains two project entries each with non-empty `highlights` or `one_line`

### Requirement: Reports can be marked read

The backend SHALL expose `PATCH /api/reports/:id/read` setting `reports.is_read=1` for the given id.

#### Scenario: Mark report read

- **WHEN** a client sends `PATCH /api/reports/42/read` for an existing unread report
- **THEN** subsequent `GET /api/people` shows decreased `unread_count` for that person

### Requirement: Web UI displays weekly reader and run controls

The frontend SHALL provide a page with a people sidebar, a main panel showing the selected person's latest weekly content, a control to trigger `POST /api/runs` with `manual_all`, and a visible notification when the latest run transitions to terminal status `success`, `partial`, or `failed`.

#### Scenario: User triggers batch run from UI

- **WHEN** the user clicks the run-all control
- **THEN** the client sends `POST /api/runs` and displays in-progress status until the run completes

#### Scenario: User marks content as read

- **WHEN** the user opens a person's report and activates mark-read
- **THEN** the client calls `PATCH /api/reports/:id/read` and the sidebar unread indicator clears for that report

