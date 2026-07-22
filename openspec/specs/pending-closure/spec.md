# pending-closure Specification

## Purpose

Managers can list and resolve open pending items from weekly reports, syncing person-level `_notes.md` so cross-month closure is visible in trends.

## Requirements

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

---
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

---
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

---
### Requirement: Weekly summary ingestion deduplicates open pending questions

When ingesting `## 待確認` bullets from a weekly `summary.md` into `pending_items`, the backend MUST skip inserting a row when an existing row already has the same `person_id`, `project_id`, and `question` with `status='open'`.

The backend MUST also skip inserting a row when an existing row with the same `person_id`, `project_id`, and `question` — in any status — originates from a report whose report date is the same as or later than the report date of the summary being ingested. This prevents a re-read of an already-processed summary from creating a duplicate row.

A previously `resolved` row with the same question MUST NOT block insertion of a new `open` row when the summary being ingested has a report date later than that of the resolved row's originating report.

When an existing row's originating report cannot be determined because its report reference is `NULL`, that row MUST NOT block insertion, and the backend MUST log a warning naming the person, project, and question.

#### Scenario: Duplicate open question is not inserted again

- **GIVEN** an open pending item exists for person P, project G, question Q
- **WHEN** a weekly summary for the same person and project is ingested containing bullet Q
- **THEN** no additional `pending_items` row is created for Q

#### Scenario: Resolved question may be raised again by a later summary

- **GIVEN** a resolved pending item exists for person P, project G, question Q, originating from a report dated D1
- **WHEN** a weekly summary for the same person and project with report date D2 later than D1 is ingested containing bullet Q
- **THEN** a new open `pending_items` row is created for Q and the resolved row remains

#### Scenario: Re-reading an already-processed summary creates no row

- **GIVEN** a pending item exists for person P, project G, question Q, originating from a report dated D1
- **WHEN** a summary for the same person and project with report date D1 is ingested again containing bullet Q
- **THEN** no additional `pending_items` row is created for Q

#### Scenario: Re-reading an older summary creates no row

- **GIVEN** a resolved pending item exists for person P, project G, question Q, originating from a report dated D2
- **WHEN** a summary for the same person and project with an earlier report date D1 is ingested containing bullet Q
- **THEN** no additional `pending_items` row is created for Q and Q does not return to `open`

#### Scenario: Missing originating report does not block insertion

- **GIVEN** a pending item exists for person P, project G, question Q whose originating report reference is `NULL`
- **WHEN** a weekly summary for the same person and project is ingested containing bullet Q
- **THEN** insertion proceeds and a warning naming person P, project G, and question Q is logged

##### Example: insertion decision by report date

| existing row status | existing row report date | incoming summary report date | new row inserted |
| ------------------- | ------------------------ | ---------------------------- | ---------------- |
| open | 2026-07-05 | 2026-07-12 | no — open row blocks |
| resolved | 2026-07-05 | 2026-07-12 | yes — incoming is later |
| resolved | 2026-07-12 | 2026-07-12 | no — same date is a re-read |
| resolved | 2026-07-12 | 2026-07-05 | no — incoming is older |
| resolved | (null reference) | 2026-07-05 | yes — cannot compare, warn |


<!-- @trace
source: pending-replay-dedup
updated: 2026-07-22
code:
  - backend/src/summary.rs
tests:
  - backend/tests/pending_items.rs
-->

---
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

---
### Requirement: Weekly ingest resolves open pending via shared closure semantics

When weekly summary ingestion resolves an open pending item because its question appears under `## 已釐清`, the system MUST apply the same closure field updates as `PATCH /api/pending-items/{id}` for an open row: set `status` to `resolved`, set `resolved_date` to the schedule-timezone month `YYYY-MM`, and leave `resolution_note` null when the summary does not supply a note.

After a successful database update, the system MUST update `{DATA_ROOT_DIR}/reports/_people/{display_name}/_notes.md` using the same resolved-line rewrite rules as manual closure.

If the notes file write fails after the database update succeeded during ingest, the pending item MUST remain `resolved`, and the ingest MUST continue (notes failure MUST NOT abort the whole project ingest).

#### Scenario: Ingest resolve rewrites matching open notes line

- **GIVEN** `_notes.md` contains `- [2026-07] Why choose A?`
- **AND** a matching open `pending_items` row exists
- **WHEN** weekly ingest resolves that item via `## 已釐清` in month `2026-07`
- **THEN** that notes line becomes `- [2026-07→2026-07] ✓ Why choose A?`

<!-- @trace
source: summary-resolved-pending
updated: 2026-07-11
code:
  - frontend/src/types.ts
  - backend/src/runs.rs
  - backend/src/schedule.rs
  - docs/idea/schema.md
  - backend/src/dashboard.rs
  - frontend/src/app.ts
  - frontend/src/style.css
  - backend/migrations/012_pending_open_by_project.sql
  - backend/src/server.rs
  - frontend/src/api.ts
  - README.md
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/summary.rs
  - skills/reviewer-batch/output-contract.md
  - backend/src/error.rs
tests:
  - backend/tests/identity.rs
  - backend/tests/schedule_api.rs
  - backend/tests/runs_execution.rs
-->