## MODIFIED Requirements

### Requirement: Person display name can be renamed

The backend SHALL expose `PATCH /api/people/{id}` accepting JSON `{ "display_name": "<string>" }`.

The new `display_name` MUST be trimmed and non-empty. Empty names MUST return HTTP 400. Duplicate names belonging to another person MUST return HTTP 409.

On success the backend MUST update `people.display_name` only. The backend MUST NOT change `people.folder_name`, MUST NOT rename any directory under `reports/_people/` or `reports/{project_name}/`, and MUST NOT rewrite stored `reports.summary_md_path` or `reports.report_md_path` values. Because report paths are keyed by the immutable `folder_name`, a rename MUST leave every existing report path valid without any filesystem operation.

The backend MUST NOT delete people via this change.

#### Scenario: Rename updates display name without moving directories

- **GIVEN** person "Alice" with `folder_name` "Alice", `reports/_people/Alice/` and `reports/crm/Alice/` present
- **WHEN** a client patches display_name to "Alice Chen"
- **THEN** `people.display_name` is "Alice Chen" and `people.folder_name` remains "Alice"
- **AND** the directories `reports/_people/Alice/` and `reports/crm/Alice/` are unchanged and not renamed

#### Scenario: Renamed person still resolves for existing reports

- **GIVEN** person "Alice" (folder_name "Alice") with a stored report under `reports/crm/Alice/`
- **WHEN** the client renames display_name to "Alice Chen"
- **THEN** the stored `reports.summary_md_path` remains valid and the report is still readable

#### Scenario: Rename rejects duplicate display name

- **GIVEN** people "Alice" and "Bob"
- **WHEN** a client patches Bob's display_name to "Alice"
- **THEN** the response status is 409
