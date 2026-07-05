## MODIFIED Requirements

### Requirement: Worker executes reviewer skill subprocess per project

For each dequeued project, the worker SHALL set the subprocess working directory to the target worktree path supplied by the `repo-worktree` capability — the resident worktree of the first `default_branches` entry for a weekly batch, or the merge-request worktree for an MR review — rather than to `projects.repo_path` directly. The worker SHALL execute `claude -p` with the configured reviewer-batch prompt and enforce timeout using `schedule_config.per_project_timeout_sec`.

If the worktree cannot be supplied (for example, the branch was deleted on the remote or fetch failed after retries), the worker MUST NOT start the subprocess for that project and MUST record the failure without aborting remaining projects.

On timeout, the worker MUST kill the subprocess, set `run_projects.state='skipped_timeout'`, and continue remaining projects.

On success, the worker MUST set `run_projects.state='done'` and record `duration_sec`.

#### Scenario: Project completes within timeout

- **WHEN** the subprocess exits with code 0 before timeout
- **THEN** the corresponding `run_projects.state` becomes `done`

#### Scenario: Project exceeds timeout

- **WHEN** the subprocess runs longer than `per_project_timeout_sec`
- **THEN** the subprocess is terminated and `run_projects.state` becomes `skipped_timeout`

#### Scenario: Subprocess runs inside the supplied worktree

- **WHEN** a project is dequeued and its target worktree path is supplied
- **THEN** the subprocess working directory is that worktree path, not `projects.repo_path`

#### Scenario: Unavailable worktree skips the subprocess

- **WHEN** the target worktree cannot be supplied because the remote branch was deleted or fetch failed after retries
- **THEN** the worker does not start the subprocess and records the failure while continuing remaining projects
