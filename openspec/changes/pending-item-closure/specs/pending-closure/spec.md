## ADDED Requirements

### Requirement: Pending items can be listed for a person

The backend SHALL expose `GET /api/people/{id}/pending-items` returning an array of pending-item objects for the given person.

Each object MUST include: `id`, `person_id`, `project_id`, `project_name`, `report_id` (nullable), `question`, `status`, `raised_date`, `resolved_date` (nullable), `resolution_note` (nullable).

The optional query parameter `status` MUST accept `open`, `resolved`, or `all`. When omitted, the default MUST be `open`.

Unknown `person_id` MUST return HTTP 404.

#### Scenario: Default list returns only open items

- **WHEN** a client calls `GET /api/people/{id}/pending-items` without a status query
- **THEN** every returned item has `status` equal to `open`
- **AND** resolved rows for that person are omitted

#### Scenario: Filter by resolved

- **WHEN** a client calls `GET /api/people/{id}/pending-items?status=resolved`
- **THEN** every returned item has `status` equal to `resolved`

#### Scenario: Unknown person returns 404

- **WHEN** a client calls `GET /api/people/{id}/pending-items` for a non-existent person id
- **THEN** the response status is 404

### Requirement: Open pending items can be resolved via API

The backend SHALL expose `PATCH /api/pending-items/{id}` accepting JSON body `{ "status": "resolved", "resolution_note"?: string }`.

On success for an `open` row, the system MUST set `status` to `resolved`, set `resolved_date` to the current calendar month `YYYY-MM` computed with `schedule_config.tz_offset_min`, store `resolution_note` when provided (or null when omitted/empty), and return HTTP 200 with the updated pending-item object.

A PATCH against a row that is already `resolved` MUST return HTTP 409 and MUST NOT modify the row.

A body whose `status` is not exactly `resolved` MUST return HTTP 400.

Unknown `{id}` MUST return HTTP 404.

Re-opening (`resolved` → `open`) MUST NOT be supported by this endpoint.

#### Scenario: Resolve an open item

- **WHEN** a client sends `PATCH /api/pending-items/{id}` with `{ "status": "resolved" }` for an open item
- **THEN** the response status is 200
- **AND** the body has `status` equal to `resolved`
- **AND** `resolved_date` matches `YYYY-MM` for the schedule timezone
- **AND** subsequent `GET /api/people` shows a decreased `open_pending_count` for that person

#### Scenario: Resolve with resolution note

- **WHEN** a client sends `PATCH /api/pending-items/{id}` with `{ "status": "resolved", "resolution_note": "Chose option B in 1on1" }`
- **THEN** the response body `resolution_note` equals `Chose option B in 1on1`

#### Scenario: Resolving an already resolved item returns 409

- **WHEN** a client sends `PATCH /api/pending-items/{id}` for an item whose status is already `resolved`
- **THEN** the response status is 409
- **AND** the database row is unchanged

#### Scenario: Invalid status value returns 400

- **WHEN** a client sends `PATCH /api/pending-items/{id}` with `{ "status": "open" }`
- **THEN** the response status is 400

### Requirement: Resolving a pending item syncs person-level notes file

After a successful database update to `resolved`, the backend MUST update `{DATA_ROOT_DIR}/reports/_people/{display_name}/_notes.md` as follows:

- Open line format: `- [YYYY-MM] {question}`
- Resolved line format: `- [YYYY-MM→YYYY-MM] ✓ {question}` with optional trailing ` — {resolution_note}` when the note is non-empty
- The writer MUST replace the first open line whose question text exactly equals the item `question`; if no such line exists, it MUST append a resolved line
- If the file or parent directories do not exist, the writer MUST create them before writing

If the notes file write fails after the database update succeeded, the endpoint MUST return HTTP 502, MUST leave the database row as `resolved`, and MUST include an error message indicating the notes file was not synced.

#### Scenario: Matching open notes line is rewritten

- **GIVEN** `_notes.md` contains `- [2026-07] Why choose A?`
- **AND** a matching open `pending_items` row exists with that question
- **WHEN** the item is resolved in month `2026-08` without a resolution note
- **THEN** that line becomes `- [2026-07→2026-08] ✓ Why choose A?`

#### Scenario: Missing matching line appends resolved entry

- **GIVEN** `_notes.md` exists but has no open line matching the question
- **WHEN** the item is resolved
- **THEN** a resolved line for that question is appended to `_notes.md`

#### Scenario: Missing notes file is created

- **GIVEN** `_people/{display_name}/` has no `_notes.md`
- **WHEN** an open pending item for that person is resolved
- **THEN** `_notes.md` is created containing a resolved line for the question

#### Scenario: Notes write failure returns 502 after DB resolve

- **GIVEN** the database update to `resolved` succeeds
- **AND** writing `_notes.md` fails
- **WHEN** the client called `PATCH /api/pending-items/{id}`
- **THEN** the response status is 502
- **AND** the pending item remains `resolved` in the database

##### Example: B1 line transformations

| Before (open line) | resolution_note | resolved_date | After |
| --- | --- | --- | --- |
| `- [2026-07] Why choose A?` | (empty) | `2026-08` | `- [2026-07→2026-08] ✓ Why choose A?` |
| `- [2026-07] Why choose A?` | `Chose B` | `2026-08` | `- [2026-07→2026-08] ✓ Why choose A? — Chose B` |

### Requirement: Weekly summary ingestion deduplicates open pending questions

When ingesting `## 待確認` bullets from a weekly `summary.md` into `pending_items`, the backend MUST skip inserting a row when an existing row already has the same `person_id`, `project_id`, and `question` with `status='open'`.

A previously `resolved` row with the same question MUST NOT block insertion of a new `open` row.

#### Scenario: Duplicate open question is not inserted again

- **GIVEN** an open pending item exists for person P, project G, question Q
- **WHEN** a weekly summary for the same person and project is ingested containing bullet Q
- **THEN** no additional `pending_items` row is created for Q

#### Scenario: Resolved question may be raised again

- **GIVEN** a resolved pending item exists for person P, project G, question Q
- **WHEN** a weekly summary for the same person and project is ingested containing bullet Q
- **THEN** a new open `pending_items` row is created for Q

### Requirement: Weekly report UI resolves pending items via checkbox

The report reader weekly project cards MUST render each open `pending_items` entry as a checkbox bound to the item `id`.

Checking a box MUST call `PATCH /api/pending-items/{id}` with `{ "status": "resolved" }`. On success, the UI MUST remove that item from the weekly card and refresh person `open_pending_count` (and dashboard pending count when the dashboard is loaded).

The trends view MUST NOT provide resolve controls for historical pending entries.

Re-opening from the UI MUST NOT be offered.

#### Scenario: Checking a pending item resolves it

- **WHEN** a manager checks an open pending checkbox on a weekly project card
- **THEN** the client sends `PATCH /api/pending-items/{id}` with status `resolved`
- **AND** on success the item disappears from the weekly card
- **AND** the people list open-pending badge decreases

