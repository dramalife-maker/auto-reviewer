# report-reader-agent-chat Specification

## Purpose

TBD - created by archiving change 'report-reader-agent-chat'. Update Purpose after archive.

## Requirements

### Requirement: Persist person report Agent Chat turns

The system SHALL store each successful person-report Agent Chat turn as two rows in `person_report_chat_messages` linked to the person: one with `role` equal to `user` and one with `role` equal to `assistant`, inserted in that order after the agent subprocess succeeds. The system MUST NOT insert chat rows when the agent turn fails. Deleting a `people` row MUST cascade-delete that person's report chat messages and chat session row.

The system SHALL keep at most one `person_report_chats` row per `person_id`, holding `agent_session_id` (nullable until a session id is parsed), `reviewer_agent`, and `updated_at`.

#### Scenario: Successful turn persists user and assistant messages

- **WHEN** `POST /api/people/1/report-chat/agent-turn` succeeds with a non-empty reply
- **THEN** exactly two new `person_report_chat_messages` rows exist for person 1 with roles `user` then `assistant`

#### Scenario: Failed turn does not persist messages

- **WHEN** the agent subprocess fails for a person-report agent-turn
- **THEN** no new `person_report_chat_messages` rows are inserted for that person


<!-- @trace
source: report-reader-agent-chat
updated: 2026-07-17
code:
  - backend/src/server.rs
  - backend/src/worker.rs
  - backend/src/reports.rs
  - backend/src/summary.rs
  - frontend/src/types.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/executor.rs
  - backend/src/report_chat.rs
  - backend/src/lib.rs
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/api.ts
  - backend/src/mr_change_materials.rs
tests:
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: Report chat API returns transcript and accepts agent turns

The backend SHALL expose `GET /api/people/:id/report-chat` returning JSON with `agent_session_id` (string or null), `reviewer_agent` (string or null when no session row), and `chat_messages` (array of `{ id, role, content, created_at }` ordered by ascending `id`). Unknown person ids MUST return HTTP 404. A known person with no prior chat MUST return `chat_messages` as an empty array and `agent_session_id` null.

The backend SHALL expose `POST /api/people/:id/report-chat/agent-turn` accepting JSON `{ "message": string }`. Empty or whitespace-only `message` MUST return HTTP 400. On success the handler MUST return HTTP 200 with `{ "reply": string, "agent_session_id": string | null }` where `reply` is the agent's final text. The success response MUST include `ingest_warnings` as a string array (empty when reingest had no problems).

When `person_report_chats.agent_session_id` is null, the handler MUST start a new agent conversation (MUST NOT pass `--resume`). When it is non-null, the handler MUST invoke the agent with `--resume` using that id and the stored `reviewer_agent`. On success the handler MUST upsert `person_report_chats` with any new session id parsed from stdout and the configured reviewer agent. Agent subprocess failure MUST return HTTP 502.

The agent invocation MUST grant filesystem access under the configured data root and the prompt MUST instruct the agent that writes are limited to that person's report directories: `reports/<project>/<display_name>/` for each project and `reports/_people/<display_name>/`. The handler MUST NOT call GitLab and MUST NOT modify MR draft files as part of this endpoint.

After a successful agent subprocess and after chat messages are persisted, the handler MUST re-ingest that person's `summary.md` files under each `reports/<project>/<display_name>/` into SQLite using the same field contracts as weekly summary ingest (`reports.one_line` / counts, open `pending_items` from `## 待確認`, and resolve via `## 已釐清`). Existing `(project_id, person_id, report_date)` rows MUST keep their previous `run_id`. Reingest failures MUST NOT change the HTTP success of the agent-turn; they MUST be logged and MUST appear in `ingest_warnings`.

#### Scenario: First turn starts a new session

- **GIVEN** person Alice has no `person_report_chats` row
- **WHEN** `POST /api/people/:id/report-chat/agent-turn` is called with a non-empty message
- **THEN** the response is 200 with a non-empty `reply`
- **AND** a `person_report_chats` row exists for Alice

#### Scenario: Later turn resumes the stored session

- **GIVEN** Alice has `person_report_chats.agent_session_id='sess-1'`
- **WHEN** a subsequent agent-turn is called
- **THEN** the agent CLI is invoked with `--resume sess-1`

#### Scenario: Get chat returns empty history for new person

- **GIVEN** person Alice exists and has never used report chat
- **WHEN** `GET /api/people/:id/report-chat` is called
- **THEN** the response is 200 with `chat_messages` equal to `[]` and `agent_session_id` null

#### Scenario: Successful turn reingests edited summary into DB

- **GIVEN** Alice has an existing `reports` row for project `game-backend` whose `summary_md_path` points at a file on disk
- **AND** that summary's frontmatter `one_line` is updated on disk during a successful agent-turn (or before reingest runs in the test harness)
- **WHEN** `POST /api/people/:id/report-chat/agent-turn` succeeds for Alice
- **THEN** the `reports` row for that project and report date reflects the updated `one_line`
- **AND** the row's `run_id` is unchanged

<!-- @trace
source: report-reader-agent-chat
updated: 2026-07-17
code:
  - backend/src/server.rs
  - backend/src/worker.rs
  - backend/src/reports.rs
  - backend/src/summary.rs
  - frontend/src/types.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/executor.rs
  - backend/src/report_chat.rs
  - backend/src/lib.rs
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/api.ts
  - backend/src/mr_change_materials.rs
tests:
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->