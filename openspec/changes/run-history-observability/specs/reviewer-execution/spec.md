## ADDED Requirements

### Requirement: Dashboard includes recent runs

The backend SHALL include `recent_runs` on `GET /api/dashboard` as an array of up to five run list items (same fields as `GET /api/runs` list items), ordered by `started_at` descending.

When no runs exist, `recent_runs` MUST be an empty array.

#### Scenario: Dashboard returns latest five runs

- **GIVEN** more than five runs exist
- **WHEN** a client calls `GET /api/dashboard`
- **THEN** `recent_runs` contains exactly five items
- **AND** they are the newest by `started_at`

