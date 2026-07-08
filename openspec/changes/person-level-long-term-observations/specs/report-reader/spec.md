## ADDED Requirements

### Requirement: Person trends API for report reader

The backend SHALL expose `GET /api/people/:id/trends` as defined in the `person-trends` capability. The report reader frontend MUST consume this endpoint when the user views the trends section for a selected person.

#### Scenario: Frontend fetches trends for selected person

- **WHEN** a user selects a person and opens the trends view
- **THEN** the client calls `GET /api/people/{id}/trends`
- **AND** renders `long_term_observation`, `growth_timeline`, and `historical_pending` sections

#### Scenario: Trends empty state

- **WHEN** trends API returns empty `long_term_observation` and empty arrays
- **THEN** the UI shows an empty-state message indicating no long-term data yet

---

## MODIFIED Requirements

### Requirement: Latest weekly report content is served per person

The backend SHALL expose `GET /api/people/:id/reports/latest` returning, for each project with a report on the person's latest `report_date`, JSON containing `project_name`, `one_line`, `mr_count`, `commit_count`, rendered sections `highlights`, `growth`, and `pending` parsed from `summary.md`.

The weekly overview API behavior MUST remain unchanged. Long-term cross-project observation MUST NOT be included in this endpoint; it is served only by the trends API.

#### Scenario: Latest reports excludes long-term observation

- **WHEN** a client calls `GET /api/people/:id/reports/latest`
- **THEN** the response contains per-project weekly cards only
- **AND** does not include person-level `index.md` content
