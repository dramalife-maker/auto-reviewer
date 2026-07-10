## ADDED Requirements

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

### Requirement: People settings UI manages persons and identities

The frontend SHALL provide a dedicated people-settings view (separate from the weekly report reader) with:

- a list of people and a control to create a new person
- an editor for the selected person's `display_name`
- identity list with add and remove controls supporting kinds `git_email`, `gitlab_user`, and `glab_user`
- a read-only list of participating project names

The existing unmatched-authors header panel MUST remain available as a shortcut.

The people-settings view MUST NOT offer a delete-person action.

#### Scenario: Create and bind identity from settings view

- **WHEN** a manager creates person "Alice Chen" and binds `git_email` `alice@co.com` from the people-settings view
- **THEN** subsequent `GET /api/people/{id}` shows that identity
- **AND** the unmatched-authors panel remains reachable from the app header

#### Scenario: Remove identity from settings view

- **WHEN** a manager removes an identity from the selected person in people-settings
- **THEN** that identity no longer appears in `GET /api/people/{id}/identities`

