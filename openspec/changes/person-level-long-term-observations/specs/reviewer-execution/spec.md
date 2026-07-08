## ADDED Requirements

### Requirement: Weekly batch manifest includes analysis window and authors

The worker SHALL write `manifest.json` under `{DATA_ROOT_DIR}/runs/{run_id}/projects/{project_id}/manifest.json` before spawning the reviewer subprocess.

The manifest MUST include fields: `mode`, `project_name`, `repo_path`, `report_root`, `run_date`, `since`, `output_contract`, and `authors`.

Each `authors[]` entry MUST include `email`, `git_name`, `person_id`, and `display_name` for resolved persons only.

The manifest MUST include `person_report_root` set to `{DATA_ROOT_DIR}/reports/_people` (the shared person-level reports root; workflow resolves per-author paths as `{person_report_root}/{display_name}/`).

#### Scenario: Manifest includes person report root

- **WHEN** a weekly batch run prepares manifest for a project with at least one resolved author
- **THEN** `manifest.json` contains `person_report_root` pointing to `reports/_people` under `DATA_ROOT_DIR`

---

## MODIFIED Requirements

### Requirement: Worker executes reviewer skill subprocess per project

For each dequeued project, the worker SHALL set the subprocess working directory to the target worktree path supplied by the `repo-worktree` capability — the resident worktree of the first `default_branches` entry for a weekly batch, or the merge-request worktree for an MR review — rather than to `projects.repo_path` directly. The worker SHALL execute the configured reviewer agent with the configured reviewer-batch prompt and enforce timeout using `schedule_config.per_project_timeout_sec`.

If the worktree cannot be supplied (for example, the branch was deleted on the remote or fetch failed after retries), the worker MUST NOT start the subprocess for that project and MUST record the failure without aborting remaining projects.

On timeout, the worker MUST kill the subprocess, set `run_projects.state='skipped_timeout'`, and continue remaining projects.

On success, the worker MUST set `run_projects.state='done'` and record `duration_sec`. After a successful subprocess exit, person-level files under `reports/_people/{display_name}/` are maintained by the subprocess per workflow contract (the worker does not parse person-level files into SQLite).

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

#### Scenario: Successful run maintains person-level files via workflow

- **GIVEN** a weekly batch subprocess completes successfully for a project with resolved author "Alice Chen"
- **WHEN** the workflow finishes
- **THEN** `reports/_people/Alice Chen/index.md` exists or is updated (append semantics per workflow)
- **AND** project-level `reports/{project}/Alice Chen/{run_date}/summary.md` exists
