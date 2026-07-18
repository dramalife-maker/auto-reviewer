# mr-agent-chat Specification

## Purpose

TBD - created by archiving change 'persist-mr-agent-chat'. Update Purpose after archive.

## Requirements

### Requirement: Persist successful Agent Chat turns in SQLite

The system SHALL store each successful MR Agent Chat turn as two rows in `mr_review_chat_messages` linked to the `mr_reviews` row: one with `role` equal to `user` (the request message) and one with `role` equal to `assistant` (the agent reply), inserted in that order after the agent subprocess succeeds. The system MUST NOT insert chat rows when the agent turn fails. Deleting an `mr_reviews` row MUST cascade-delete its chat messages.

#### Scenario: Successful turn stores user then assistant

- **WHEN** `POST /api/mr-reviews/:id/agent-turn` succeeds for a draft review with a non-empty `agent_session_id`
- **THEN** the database contains a new `user` message with the request text and a new `assistant` message with the reply text for that review id
- **AND** the assistant row id is greater than the user row id

#### Scenario: Failed turn leaves transcript unchanged

- **WHEN** `POST /api/mr-reviews/:id/agent-turn` fails after the agent subprocess errors or times out
- **THEN** no new rows are inserted into `mr_review_chat_messages` for that review


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: List API returns chat transcript per review

`GET /api/mr-reviews` SHALL include a `chat_messages` array on every list item. Each element MUST contain `id`, `role` (`user` or `assistant`), `content`, and `created_at`. Messages MUST be ordered by ascending `id`. Reviews with no chat history MUST return an empty array. Each list item MUST also include `draft_hash`, the SHA-256 hex digest of that item's strip-frontmatter `draft_body`.

#### Scenario: Reload restores prior chat

- **WHEN** a review has previously stored chat messages and the client calls `GET /api/mr-reviews` for that review status
- **THEN** the matching list item includes those messages in ascending `id` order
- **AND** the item includes a non-empty `draft_hash`

##### Example: two-turn transcript shape

- **GIVEN** messages `(id=1, role=user, content=why flag helper?)` and `(id=2, role=assistant, content=because it wraps commits)`
- **WHEN** the client lists that review
- **THEN** `chat_messages` equals `[{id:1,role:"user",...},{id:2,role:"assistant",...}]`


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: Publish and ignore retain chat history

Changing an MR review status to `published` or `ignored` MUST NOT delete its `mr_review_chat_messages`. Agent turn requests for non-draft reviews MUST continue to be rejected as conflicts.

#### Scenario: Published review keeps transcript

- **WHEN** a draft with stored chat messages is published successfully
- **THEN** `GET /api/mr-reviews?status=published` for that review still returns the same `chat_messages`


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: Agent turn returns current draft body

After a successful agent subprocess, `POST /api/mr-reviews/:id/agent-turn` MUST re-read the file at `draft_md_path`, strip frontmatter, and include the resulting text as `draft_body` plus its `draft_hash` in the HTTP 200 response together with `reply` and `agent_session_id`. The handler MUST NOT itself rewrite the draft file; agent-driven edits on disk are allowed and MUST be reflected in the returned `draft_body`.

#### Scenario: Response includes re-read draft

- **WHEN** `POST /api/mr-reviews/:id/agent-turn` succeeds and the draft file body differs from the pre-turn content
- **THEN** the response `draft_body` equals the strip-frontmatter file content after the turn
- **AND** `draft_hash` matches that body

#### Scenario: Unchanged draft still returned

- **WHEN** `POST /api/mr-reviews/:id/agent-turn` succeeds and the draft file is unchanged
- **THEN** the response still includes `draft_body` and `draft_hash` for the current file


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: Draft PATCH supports optimistic base hash

`PATCH /api/mr-reviews/:id` SHALL accept optional `base_hash`. When `base_hash` is present and does not equal the SHA-256 hex of the current strip-frontmatter draft file body, the handler MUST NOT write the file and MUST respond with HTTP 409 including the current `draft_body` and `draft_hash`. When `base_hash` is omitted, the handler MUST overwrite as today. When `base_hash` matches, the handler MUST write `draft_body` and succeed.

#### Scenario: Stale base hash rejected

- **WHEN** the client PATCHes with `base_hash` that does not match the on-disk draft body hash
- **THEN** the response is HTTP 409
- **AND** the on-disk draft file is unchanged
- **AND** the response includes the current `draft_body` and `draft_hash`

#### Scenario: Matching base hash saves

- **WHEN** the client PATCHes a draft with `base_hash` equal to the current file hash and a new `draft_body`
- **THEN** the file is updated to the new body
- **AND** the response is success

<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->