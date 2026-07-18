## ADDED Requirements

### Requirement: MR triage script enumerates and filters merge requests before agents run

Before spawning any MR review agent subprocess, the backend SHALL execute `scripts/triage-mrs.py` with the scan manifest path. The script MUST run with working directory set to the project's resident worktree and MUST invoke `glab` only (no AI agent, no SQLite).

The script MUST:
1. List open merge requests for the project via `glab`.
2. For each merge request, determine `review_round` (1 or 2) and whether review is needed, using deterministic rules aligned with `spec.md §6.4`: round 1 when no GitLab note contains `By: AI Agent`; round 2 when such a note exists and there is new activity since that note; skip when there is no new activity since the last AI note.
3. Before round/dedup logic, apply **readiness gates** from manifest: skip GitLab draft MRs; skip MRs bearing any label in `mr_review_skip_labels`; when `mr_review_require_label` is set, skip MRs that do not bear that label.
4. Write `eligible_mrs.json` next to the manifest with `eligible[]` (MRs to review) and `skipped[]` (MRs excluded with `skip_reason`).

Each `eligible[]` entry MUST include `mr_iid`, `mr_title`, `source_branch`, `target_branch`, `author_identity`, and `review_round`.

The backend MUST read `eligible[]` and spawn agent subprocesses only for those entries, **one at a time in queue order** within the project. When `eligible` is empty, the backend MUST mark the run project `done` without spawning any agent.

When the triage script exits non-zero or produces unparseable output, the backend MUST mark the run project `failed` and MUST NOT spawn agents.

#### Scenario: No eligible MRs completes without agent subprocesses

- **WHEN** triage writes `eligible: []` for a project scan
- **THEN** the run project finishes with `state='done'` and zero agent subprocesses are spawned

#### Scenario: Triage failure prevents agent spawn

- **WHEN** `triage-mrs.py` exits non-zero
- **THEN** the run project is marked `failed` and no agent subprocess is spawned

#### Scenario: Only eligible MRs receive agent subprocesses

- **WHEN** triage writes `eligible` with MR iids 42 and 55 and `skipped` with MR iid 7
- **THEN** exactly two agent subprocesses are spawned, scoped to iids 42 and 55, and the second starts only after the first exits

#### Scenario: Draft merge request is skipped before review

- **WHEN** MR iid 9 is marked as a GitLab draft and triage runs
- **THEN** iid 9 appears in `skipped` with `skip_reason='gitlab_draft'` and no agent subprocess is spawned for it

#### Scenario: Merge request with excluded label is skipped

- **WHEN** MR iid 11 has label `wip` and manifest `mr_review_skip_labels` includes `wip`
- **THEN** iid 11 appears in `skipped` with `skip_reason='label:wip'`

#### Scenario: Merge request without required label is skipped

- **WHEN** manifest `mr_review_require_label` is `ready-for-review` and MR iid 15 has no such label
- **THEN** iid 15 appears in `skipped` with `skip_reason='missing_required_label:ready-for-review'`

### Requirement: MR draft is parsed and upserted into the inbox

After a scan subprocess completes (either scheduled `mr_poll` or manual `manual_mr_poll`), the backend SHALL scan draft files written under the manifest's `draft_dir`. For each draft file, the parser MUST read frontmatter fields `mr_iid`, `mr_title`, `review_round`, and `author_identity`, resolve `person_id` by matching `author_identity` against `person_identities`, and upsert a row into `mr_reviews` keyed by `(project_id, mr_iid, review_round)` with `status='draft'`.

The upsert MUST also persist `agent_session_id` and `reviewer_agent` captured from that MR's scan subprocess stdout (see Requirement: MR scan subprocess persists agent session). When re-parsing the same `(project_id, mr_iid, review_round)`, the backend MUST update `agent_session_id` and `reviewer_agent` to the latest scan's values.

If a draft file is missing `mr_iid` or `review_round`, the parser MUST skip that file, log a warning, and continue parsing remaining files in the same `draft_dir`.

If `author_identity` does not match any `person_identities` row, the parser MUST still insert or update the `mr_reviews` row with `person_id=NULL`.

#### Scenario: New draft creates an inbox row

- **WHEN** a draft file with `mr_iid: 42`, `review_round: 1`, `author_identity: alice@co.com` is parsed and no existing `mr_reviews` row matches `(project_id, 42, 1)`
- **THEN** a new `mr_reviews` row is inserted with `status='draft'` and `person_id` resolved to Alice

#### Scenario: Re-parsing the same MR and round updates in place

- **WHEN** a draft file for `(project_id, mr_iid=42, review_round=1)` is parsed a second time after a later scan that produced a new `agent_session_id`
- **THEN** the existing `mr_reviews` row is updated (`draft_md_path`, `agent_session_id`, `reviewer_agent`, `updated_at`) and no second row is created

#### Scenario: Draft missing required frontmatter is skipped

- **WHEN** a draft file has no `mr_iid` field
- **THEN** that file is skipped, a warning is logged, and other draft files in the same run are still parsed

### Requirement: MR scan subprocess persists agent session per draft

For each merge request reviewed during a scan, the backend SHALL spawn a dedicated headless agent subprocess scoped to that MR (manifest includes `mr_iid`). The subprocess MUST enable provider session persistence (Claude: omit `--no-session-persistence`; Cursor: default behavior).

After the subprocess exits successfully, the backend MUST parse its `stream-json` stdout and extract `agent_session_id`. The backend MUST record `agent_session_id` and the configured `reviewer_agent` (`claude` or `cursor`) on the corresponding `mr_reviews` row when ingesting that MR's draft.

If the subprocess succeeds but no `session_id` can be parsed from stdout, the backend MUST still ingest the draft with `agent_session_id=NULL` and log a warning.

#### Scenario: Successful MR scan binds session to draft row

- **WHEN** a per-MR scan subprocess exits 0 and its stdout contains `session_id` `abc-123`
- **THEN** the ingested `mr_reviews` row for that MR has `agent_session_id='abc-123'` and `reviewer_agent` matching the server's `REVIEWER_AGENT` setting

#### Scenario: Draft ingested without session when stdout lacks session id

- **WHEN** a per-MR scan subprocess exits 0 but stdout contains no parseable `session_id`
- **THEN** the draft row is inserted or updated with `agent_session_id=NULL`

### Requirement: MR review inbox lists draft entries

The backend SHALL expose `GET /api/mr-reviews` accepting an optional `status` query parameter (`draft` | `published` | `ignored`, default `draft`) and returning an array of objects with fields `id`, `project_id`, `project_name`, `person_id`, `author_name`, `mr_iid`, `mr_title`, `review_round`, `status`, `draft_body`, `agent_session_id`, `reviewer_agent`, `created_at`, ordered by `created_at` descending.

#### Scenario: Inbox returns only draft status by default

- **WHEN** a client calls `GET /api/mr-reviews` with no query parameter and the database has two `draft` rows and one `published` row for the same project
- **THEN** the response array contains exactly the two `draft` rows

### Requirement: Draft content can be edited before publishing

The backend SHALL expose `PATCH /api/mr-reviews/:id` accepting JSON `{ "draft_body": string }`. The handler MUST overwrite the content of the file at `draft_md_path` with the provided body and MUST NOT call any GitLab API. The handler MUST reject the request with HTTP 409 if the target row's `status` is not `draft`.

#### Scenario: Editing a draft updates the file content

- **WHEN** a client patches `mr_reviews` row id 7 (status `draft`) with `{ "draft_body": "revised text" }`
- **THEN** the file at that row's `draft_md_path` contains `revised text` and the row remains `status='draft'`

#### Scenario: Editing a published review is rejected

- **WHEN** a client patches a `mr_reviews` row with `status='published'`
- **THEN** the server responds with HTTP 409 and does not modify the file

### Requirement: Publishing a draft posts to GitLab and records the published body

The backend SHALL expose `POST /api/mr-reviews/:id/publish`. The handler MUST invoke `glab mr note <mr_iid> --message <draft_body>` with working directory set to the project's resident worktree. On success, the handler MUST set `status='published'`, `published_at`, and `published_body` to the posted content. On failure, the handler MUST respond with HTTP 502 and MUST leave `status='draft'` unchanged.

#### Scenario: Successful publish updates status and records body

- **WHEN** `POST /api/mr-reviews/7/publish` is called for a `draft` row and `glab mr note` exits 0
- **THEN** the row becomes `status='published'` with `published_at` set and `published_body` equal to the note content posted

#### Scenario: Failed publish leaves the draft unchanged

- **WHEN** `glab mr note` exits non-zero (for example, the MR was closed)
- **THEN** the server responds 502 and the row remains `status='draft'`

### Requirement: Ignoring a draft never contacts GitLab

The backend SHALL expose `POST /api/mr-reviews/:id/ignore`. The handler MUST set `status='ignored'` and MUST NOT invoke any GitLab command.

#### Scenario: Ignoring a draft changes status only

- **WHEN** `POST /api/mr-reviews/12/ignore` is called for a `draft` row
- **THEN** the row becomes `status='ignored'` and no `glab` command is invoked

### Requirement: Draft agent session can be continued for clarification

The backend SHALL expose `POST /api/mr-reviews/:id/agent-turn` accepting JSON `{ "message": string }`. When the target row has `status='draft'` and a non-null `agent_session_id`, the handler MUST invoke the configured agent CLI with `--resume <agent_session_id>` and the provided message, using the same `reviewer_agent` recorded on the row. On success, the handler MUST return HTTP 200 with `{ "reply": string, "agent_session_id": string }` where `reply` is the agent's final text response and `agent_session_id` is the (possibly unchanged) session token.

The handler MUST NOT modify `draft_md_path`, `status`, or call any GitLab command. The handler MUST respond with HTTP 409 when `status` is not `draft` or `agent_session_id` is null. The handler MUST respond with HTTP 502 when the agent subprocess fails.

#### Scenario: Agent turn returns a reply for a draft with session

- **WHEN** `POST /api/mr-reviews/7/agent-turn` is called with `{ "message": "Why did you flag the transaction helper?" }` for a `draft` row with `agent_session_id='sess-1'`
- **THEN** the response is 200 with a non-empty `reply` and `agent_session_id='sess-1'`

#### Scenario: Agent turn rejected without session

- **WHEN** `POST /api/mr-reviews/7/agent-turn` is called for a `draft` row with `agent_session_id=NULL`
- **THEN** the server responds with HTTP 409

#### Scenario: Agent turn rejected for published review

- **WHEN** `POST /api/mr-reviews/7/agent-turn` is called for a row with `status='published'`
- **THEN** the server responds with HTTP 409

### Requirement: Observation snippets are consumed by the weekly track only after publish

The scan workflow SHALL write per-engineer observation snippet files under `reports/<project>/<person>/_pending/` for every scanned MR, independent of draft status. The weekly `reviewer-batch` workflow MUST only fold snippets into that week's `summary.md` when the corresponding `mr_reviews` row has `status='published'`; snippets whose `mr_reviews` row is `draft` or `ignored` MUST remain in `_pending/` unconsumed.

#### Scenario: Published review snippet is folded into the weekly summary

- **WHEN** a weekly batch run executes and a snippet in `_pending/` corresponds to an `mr_reviews` row with `status='published'`
- **THEN** the snippet content is folded into that week's `summary.md` and removed from `_pending/`

#### Scenario: Draft-status snippet is left untouched

- **WHEN** a weekly batch run executes and a snippet in `_pending/` corresponds to an `mr_reviews` row with `status='draft'`
- **THEN** the snippet remains in `_pending/` and is not folded into `summary.md`
