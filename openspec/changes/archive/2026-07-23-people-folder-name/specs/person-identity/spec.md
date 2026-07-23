## ADDED Requirements

### Requirement: People have an immutable folder_name path key

The `people` table SHALL include a `folder_name` column that is NOT NULL and UNIQUE. `folder_name` MUST be set once when a person is created and MUST equal the person's initial `display_name` (trimmed). No API MUST change `folder_name` after creation.

`folder_name` SHALL be the single stable key used for all on-disk report paths and for resolving a person from summary output. `display_name` MUST NOT be used as an on-disk path segment or as the ingest resolution key.

For rows that exist before this capability is introduced, `folder_name` MUST be backfilled from the current `display_name`.

#### Scenario: Existing rows backfill folder_name from display_name

- **GIVEN** a `people` row with `display_name` "Alice Chen" created before this capability
- **WHEN** the folder_name column is introduced
- **THEN** that row's `folder_name` is "Alice Chen"

#### Scenario: folder_name is not mutated by any API

- **GIVEN** person with `folder_name` "Alice"
- **WHEN** any people API (create, rename, bind identity) is invoked for that person
- **THEN** `folder_name` remains "Alice"

## MODIFIED Requirements

### Requirement: Create person API

The backend SHALL expose `POST /api/people` accepting JSON `{ "display_name": "<string>" }` and returning `{ "id": <number>, "display_name": "<string>" }`.

The `display_name` MUST be unique among `people` rows. Duplicate names MUST be rejected with HTTP 409.

On creation the backend MUST set `folder_name` equal to the trimmed initial `display_name`. `folder_name` MUST be immutable thereafter.

#### Scenario: Create a new person

- **WHEN** a client posts `{ "display_name": "Alice Chen" }` and no person with that name exists
- **THEN** the response status is 201 and a new `people` row exists

#### Scenario: New person folder_name equals initial display_name

- **WHEN** a client posts `{ "display_name": "Alice Chen" }`
- **THEN** the created row has `folder_name` equal to "Alice Chen"
