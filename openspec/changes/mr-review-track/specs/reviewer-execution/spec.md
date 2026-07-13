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

When executing an MR scan (`mr_poll` or `manual_mr_poll`), the backend SHALL run `scripts/triage-mrs.py` first, then spawn one headless agent subprocess **per** entry in `eligible_mrs.json`, **sequentially** (MR N+1 MUST NOT start until MR N's agent subprocess has finished — one agent at a time within the project). Each subprocess MUST use `stream-json` output and MUST enable provider session persistence (Claude: MUST NOT pass `--no-session-persistence`; Cursor: default session behavior).

The worker MUST atomically claim each `run_projects` row when dequeuing (transition `queued` → `running` in the same step as selecting the next job) so a single queued project cannot be spawned more than once.

The executor MUST capture stdout from each per-MR subprocess and make it available to the MR draft ingestion step for `session_id` extraction. Weekly batch execution (`schedule`, `manual_all`, `manual_project`) MUST continue to use `--no-session-persistence` for Claude and MUST NOT change behavior.

The scan-mrs-headless workflow MUST NOT invoke `glab mr list` for merge request discovery; triage is complete before the agent starts. Before each per-MR agent subprocess, the worker MUST precompute change materials inside the provisioned MR worktree against `origin/<target_branch>...HEAD` (`git fetch`, `git log --oneline`, `git diff --stat`, and a size-capped `git diff` soft-capped so a single tool Read can consume the overview), write them under the run project directory, and expose absolute paths on the MR manifest as `change_log_path`, `change_stat_path`, and `change_diff_path`. The workflow MUST gather the change set primarily from `change_stat` / `change_log`, MUST NOT page through the entire `change.diff`, and MUST prefer Reading a bounded set of changed source files in the worktree. The workflow MUST NOT invoke `glab mr diff`. The workflow MUST NOT re-run a full `git fetch` / `git log origin/<target_branch>...HEAD` / `git diff origin/<target_branch>...HEAD` when the precomputed paths are present; it MAY run a single-path `git diff origin/<target_branch>...HEAD -- <path>` when needed. The workflow MAY invoke `glab mr view` for the single `mr_iid` specified in the manifest when gathering discussion / description context that is not available from git. The workflow MUST write the MR draft before the observation snippet and MUST NOT broadly Glob `pending_dir` or `reports/`.

#### Scenario: Eligible MRs in one project run one agent at a time

- **WHEN** triage writes `eligible` with MR iids 42 and 55 for a single project
- **THEN** the worker spawns exactly two agent subprocesses and the second MUST start only after the first has exited

#### Scenario: Worker precomputes change materials before the agent

- **WHEN** an eligible MR with non-empty `target_branch` is about to be reviewed
- **THEN** the worker writes `change_log.txt`, `change_stat.txt`, and `change.diff` for `origin/<target_branch>...HEAD` and the per-MR manifest includes `change_log_path`, `change_stat_path`, and `change_diff_path`

#### Scenario: Queued run project is claimed at most once

- **WHEN** the worker drain loop dequeues while `max_concurrency` allows multiple tasks
- **THEN** each `run_projects` row in `state='queued'` is transitioned to `running` by at most one claim and is not processed by two concurrent `process_run_project` tasks

#### Scenario: Weekly batch still disables session persistence

- **WHEN** a weekly batch subprocess is built for Claude
- **THEN** the command includes `--no-session-persistence`

#### Scenario: MR scan subprocess omits session persistence disable flag

- **WHEN** a per-MR MR scan subprocess is built for Claude
- **THEN** the command does not include `--no-session-persistence`

#### Scenario: Triage runs before any MR agent subprocess

- **WHEN** an MR scan run project begins execution
- **THEN** `triage-mrs.py` completes and writes `eligible_mrs.json` before the first agent subprocess is spawned
