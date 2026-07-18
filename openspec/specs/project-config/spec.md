# project-config Specification

## Purpose

TBD - created by archiving change 'cloud-reviewer-mvp'. Update Purpose after archive.

## Requirements

### Requirement: Projects load from YAML at startup

The backend SHALL load project definitions from a YAML file at repository root named `projects.yaml` unless overridden by environment variable `PROJECTS_CONFIG`.

Each entry MUST include `name`, `repo_path`, and `git_remote_url`. Each entry MUST include `default_branches` as a non-empty list of branch names identifying the resident worktrees to provision. An entry missing `git_remote_url` MUST be stored and the corresponding project marked unhealthy rather than aborting the load.

Before upserting, the server SHALL resolve each entry's `repo_path` using `DATA_ROOT_DIR`:

- If `repo_path` is an absolute filesystem path, the server MUST store it unchanged.
- If `repo_path` is a relative path whose first component is `.` or `..`, the server MUST treat it as an explicit path relative to the process current working directory and store it unchanged.
- Otherwise, the server MUST resolve `repo_path` as `{DATA_ROOT_DIR}/repos/{repo_path}` and store the resolved path.

The resolved `repo_path` denotes a bare-plus-worktree container directory (holding `.bare/` and worktrees), not a pre-existing git working copy.

On load, the server SHALL upsert rows in the `projects` table keyed by unique `name`, using the **resolved** `repo_path` value.

#### Scenario: Load two projects from YAML

- **WHEN** `projects.yaml` contains two valid project entries with resolved `repo_path` values
- **THEN** the `projects` table contains exactly two rows with matching `name` and resolved `repo_path` values

#### Scenario: Repo slug resolves under data root repos

- **WHEN** `DATA_ROOT_DIR` is `/data/reviewer` and an entry has `repo_path: test/projectA`
- **THEN** the stored `repo_path` is `/data/reviewer/repos/test/projectA` (platform-native path separators allowed)

#### Scenario: Absolute repo path is unchanged

- **WHEN** an entry has `repo_path: /srv/git/projectA`
- **THEN** the stored `repo_path` is `/srv/git/projectA`

#### Scenario: Explicit relative path is unchanged

- **WHEN** an entry has `repo_path: ./custom/repos/projectA`
- **THEN** the stored `repo_path` is `./custom/repos/projectA` (not prefixed with `DATA_ROOT_DIR/repos`)

#### Scenario: Entry without remote URL is marked unhealthy

- **WHEN** an entry omits `git_remote_url`
- **THEN** the project row is stored and the project is marked unhealthy, and other entries still load


<!-- @trace
source: repo-worktree-layout
updated: 2026-07-05
code:
  - backend/src/worker.rs
  - backend/src/runs.rs
  - backend/migrations/002_project_health.sql
  - backend/src/lib.rs
  - backend/src/executor.rs
  - projects.yaml
  - backend/src/projects.rs
  - backend/src/worktree.rs
  - .env.example
  - README.md
tests:
  - backend/tests/project_config.rs
  - backend/tests/worktree.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: Git repository detection updates project metadata

For each loaded project, the backend SHALL provision the bare repository and resident worktrees (see the `repo-worktree` capability) and record whether provisioning succeeded. When the bare repository is present, the backend SHALL store `is_git_repo=1` and `default_branch` set to the first entry of `default_branches`. When provisioning fails or the project is unhealthy, the backend SHALL store `is_git_repo=0` and keep the project stored for later correction.

#### Scenario: Successful provisioning marks project as git repo

- **WHEN** a project's bare repository is provisioned and its resident worktrees are created
- **THEN** the project row has `is_git_repo=1` and `default_branch` equal to the first `default_branches` entry

#### Scenario: Failed provisioning keeps project for correction

- **WHEN** provisioning fails for a project (for example, unreachable remote)
- **THEN** the project row has `is_git_repo=0` and the project remains stored

<!-- @trace
source: repo-worktree-layout
updated: 2026-07-05
code:
  - backend/src/worker.rs
  - backend/src/runs.rs
  - backend/migrations/002_project_health.sql
  - backend/src/lib.rs
  - backend/src/executor.rs
  - projects.yaml
  - backend/src/projects.rs
  - backend/src/worktree.rs
  - .env.example
  - README.md
tests:
  - backend/tests/project_config.rs
  - backend/tests/worktree.rs
  - backend/tests/runs_execution.rs
-->

---
### Requirement: Projects configure MR review readiness gates

Each project MAY configure how the MR triage script excludes merge requests that are not ready for AI review. The backend SHALL store these fields on the `projects` table and include them in the `mr_poll` manifest passed to `triage-mrs.py`:

- `mr_review_skip_labels`: JSON array of label names. When an open MR bears any listed label (case-insensitive match), triage MUST skip it with `skip_reason='label:<name>'`.
- `mr_review_require_label`: optional string. When non-null, an open MR MUST bear this label to enter `eligible`; otherwise triage MUST skip it with `skip_reason='missing_required_label:<name>'`.

When `mr_review_skip_labels` is unset at load time, the backend MUST default it to `["wip", "do-not-review", "no-ai-review"]`.

Triage MUST always skip GitLab draft merge requests regardless of label configuration, with `skip_reason='gitlab_draft'`.

Projects loaded from `projects.yaml` MAY specify `mr_review_skip_labels` and `mr_review_require_label` per entry; values MUST be upserted into the `projects` table on load.

#### Scenario: Default skip labels apply when project omits configuration

- **WHEN** a project has no `mr_review_skip_labels` in YAML or the database
- **THEN** the stored value defaults to `["wip", "do-not-review", "no-ai-review"]` and the MR poll manifest includes that array

#### Scenario: Project overrides skip labels via YAML

- **WHEN** `projects.yaml` sets `mr_review_skip_labels: ["wip"]` for project `game-backend`
- **THEN** after load the project row stores that array and only `wip` triggers label-based skips for that project

#### Scenario: Require label is passed through manifest

- **WHEN** a project has `mr_review_require_label='ready-for-review'`
- **THEN** the `mr_poll` manifest for that project includes `mr_review_require_label: "ready-for-review"`

<!-- @trace
source: mr-review-track
updated: 2026-07-17
code:
  - .spectra.yaml
  - backend/src/server.rs
  - .kiro/prompts/spectra-commit.prompt.md
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/main.tsx
  - backend/src/lib.rs
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - backend/src/schedule.rs
  - frontend/src/hooks/useApi.ts
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/lib/icons.ts
  - .kiro/skills/spectra-drift/SKILL.md
  - skills/scan-mrs-headless/WORKFLOW.md
  - .kiro/prompts/spectra-audit.prompt.md
  - frontend/src/lib/format.ts
  - frontend/package.json
  - frontend/vite.config.ts
  - frontend/src/components/ui/Input.tsx
  - backend/src/report_chat.rs
  - .kiro/skills/spectra-archive/SKILL.md
  - frontend/src/components/ui/Card.tsx
  - .kiro/skills/spectra-apply/SKILL.md
  - frontend/src/components/ui/Avatar.tsx
  - backend/src/state.rs
  - .kiro/skills/spectra-commit/SKILL.md
  - skills/scan-mrs-headless/observation-guidelines.md
  - frontend/src/pages/MrInboxPage.tsx
  - backend/src/pending_items.rs
  - frontend/src/components/ui/Button.tsx
  - scripts/triage-mrs.py
  - frontend/src/style.css
  - .kiro/prompts/spectra-propose.prompt.md
  - backend/src/projects.rs
  - frontend/tsconfig.json
  - backend/src/worktree.rs
  - skills/reviewer-batch/WORKFLOW.md
  - .kiro/prompts/spectra-archive.prompt.md
  - backend/migrations/006_mr_review_agent_session.sql
  - backend/src/person_trends.rs
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Tabs.tsx
  - .kiro/skills/spectra-discuss/SKILL.md
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/pages/DashboardPage.tsx
  - backend/migrations/012_pending_open_by_project.sql
  - docs/idea/roadmap-workflow-growth.md
  - .kiro/skills/spectra-debug/SKILL.md
  - backend/src/worker.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/components/layout/Toast.tsx
  - backend/Cargo.toml
  - frontend/src/types.ts
  - .kiro/prompts/spectra-drift.prompt.md
  - .kiro/skills/spectra-audit/SKILL.md
  - frontend/src/index.css
  - frontend/src/lib/catchup.ts
  - frontend/src/lib/tokens.ts
  - frontend/src/components/ui/NavItem.tsx
  - README.md
  - frontend/index.html
  - frontend/src/components/layout/Sidebar.tsx
  - .kiro/skills/spectra-ingest/SKILL.md
  - backend/src/runs.rs
  - .kiro/prompts/spectra-apply.prompt.md
  - backend/src/summary.rs
  - frontend/src/App.tsx
  - frontend/src/components/ui/index.ts
  - frontend/src/components/ui/StatCard.tsx
  - skills/scan-mrs-headless/output-contract.md
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/src/app.ts
  - frontend/src/components/ui/Badge.tsx
  - .kiro/prompts/spectra-debug.prompt.md
  - frontend/src/pages/PeoplePage.tsx
  - .kiro/prompts/spectra-discuss.prompt.md
  - frontend/src/components/ui/ConfirmDialog.tsx
  - frontend/src/main.ts
  - docs/idea/schema.md
  - backend/migrations/011_runs_filter_indexes.sql
  - backend/src/reports.rs
  - backend/src/error.rs
  - backend/migrations/013_mr_review_chat_messages.sql
  - backend/migrations/009_mr_reviews_project_status_index.sql
  - frontend/src/pages/RunsPage.tsx
  - backend/migrations/007_mr_review_project_gates.sql
  - skills/project-adr-notes/SKILL.md
  - .kiro/prompts/spectra-ingest.prompt.md
  - backend/src/executor.rs
  - frontend/src/pages/ProjectsPage.tsx
  - backend/src/config.rs
  - backend/src/identity.rs
  - backend/migrations/014_person_report_chat.sql
  - .kiro/prompts/spectra-ask.prompt.md
  - backend/migrations/008_mr_scan_force.sql
  - backend/src/mr_reviews.rs
  - backend/migrations/010_pending_items_indexes.sql
  - backend/src/dashboard.rs
  - .kiro/skills/spectra-ask/SKILL.md
  - docs/design_handoff_reviewer_redesign/support.js
  - .kiro/skills/spectra-propose/SKILL.md
  - frontend/src/context/ToastContext.tsx
  - backend/src/mr_change_materials.rs
  - frontend/src/api.ts
tests:
  - backend/tests/foundation.rs
  - backend/tests/fixtures/flood_stdout.sh
  - backend/tests/fixtures/write_draft_then_hang.sh
  - backend/tests/fixtures/fake_triage_eligible.py
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - backend/tests/fixtures/report_chat_fail.cmd
  - frontend/src/hooks/useApi.test.ts
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/write_draft_then_hang.cmd
  - backend/tests/identity.rs
  - backend/tests/fixtures/write_draft_then_hang.py
  - backend/tests/fixtures/agent_turn_fail.sh
  - frontend/src/theme.test.ts
  - frontend/src/pages/MrInboxPage.test.tsx
  - backend/tests/fixtures/flood_stdout.cmd
  - backend/tests/schedule_api.rs
  - scripts/test_triage_mrs.py
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/lib/catchup.test.ts
  - backend/tests/fixtures/agent_turn_fail.cmd
  - backend/tests/mr_reviews.rs
  - frontend/src/components/ui/atoms.test.tsx
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_reader.rs
  - backend/tests/executor_cancellation.rs
  - backend/tests/fixtures/slow_executor.sh
  - frontend/src/pages/RunsPage.test.tsx
  - backend/tests/person_trends.rs
  - backend/tests/pending_items.rs
  - frontend/src/App.routes.test.tsx
  - backend/tests/fixtures/agent_turn_ok.py
  - backend/tests/graceful_shutdown.rs
  - backend/tests/runs_execution.rs
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/fixtures/agent_turn_ok.sh
  - frontend/src/components/layout/Toast.test.tsx
  - backend/tests/fixtures/agent_turn_ok.cmd
  - backend/tests/scheduling.rs
  - backend/tests/fixtures/flood_stdout.py
  - frontend/src/test/setup.ts
  - backend/tests/dashboard.rs
-->