## ADDED Requirements

### Requirement: Manual MR scan enqueues a single project

The backend SHALL expose `POST /api/projects/:id/mr-scan`. The handler MUST create a `runs` row with `trigger='manual_mr_poll'` and `status='running'`, insert one `run_projects` row for the target project with `state='queued'`, and enqueue work for the worker pool using the `mr_poll` manifest mode and the `scan-mrs-headless` workflow.

If the target project already has a `run_projects` row with `state` in `('queued','running')` for any active run (from either track), the server MUST reject the request with HTTP 409.

#### Scenario: Manual MR scan starts for an idle project

- **WHEN** a client posts `POST /api/projects/5/mr-scan` and project 5 has no queued or running `run_projects` row
- **THEN** the response includes a run id and project 5 appears in `run_projects` with `state='queued'` under a `runs` row with `trigger='manual_mr_poll'`

#### Scenario: Manual MR scan is rejected while a weekly run is in progress

- **WHEN** a client posts `POST /api/projects/5/mr-scan` while project 5 already has a `run_projects` row with `state='running'` under a `trigger='manual_project'` run
- **THEN** the server responds with HTTP 409 and no new run is created

### Requirement: MR scan subprocess enables agent session persistence

When executing an MR scan (`mr_poll` or `manual_mr_poll`), the backend SHALL run `scripts/triage-mrs.py` first, then spawn one headless agent subprocess per entry in `eligible_mrs.json`. Each subprocess MUST use `stream-json` output and MUST enable provider session persistence (Claude: MUST NOT pass `--no-session-persistence`; Cursor: default session behavior).

The executor MUST capture stdout from each per-MR subprocess and make it available to the MR draft ingestion step for `session_id` extraction. Weekly batch execution (`schedule`, `manual_all`, `manual_project`) MUST continue to use `--no-session-persistence` for Claude and MUST NOT change behavior.

The scan-mrs-headless workflow MUST NOT invoke `glab mr list` for merge request discovery; triage is complete before the agent starts. The workflow MAY invoke `glab mr diff` or `glab mr view` for the single `mr_iid` specified in the manifest when gathering review material.

#### Scenario: Weekly batch still disables session persistence

- **WHEN** a weekly batch subprocess is built for Claude
- **THEN** the command includes `--no-session-persistence`

#### Scenario: MR scan subprocess omits session persistence disable flag

- **WHEN** a per-MR MR scan subprocess is built for Claude
- **THEN** the command does not include `--no-session-persistence`

#### Scenario: Triage runs before any MR agent subprocess

- **WHEN** an MR scan run project begins execution
- **THEN** `triage-mrs.py` completes and writes `eligible_mrs.json` before the first agent subprocess is spawned
