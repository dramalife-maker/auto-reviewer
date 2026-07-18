# repo-worktree Specification

## Purpose

TBD - created by archiving change 'repo-worktree-layout'. Update Purpose after archive.

## Requirements

### Requirement: Bare repository and resident worktrees are provisioned at startup

When project definitions are loaded, the backend SHALL idempotently provision each project that has a `git_remote_url`. Provisioning MUST create `{repo_path}/.bare/` via `git clone --bare <git_remote_url> .bare` when it does not exist, and MUST create one worktree under `{repo_path}/<escaped-branch>/` for each branch listed in `default_branches` when that worktree does not exist.

After cloning the bare repository, the backend MUST configure the fetch refspec `+refs/heads/*:refs/remotes/origin/*` on the `origin` remote so subsequent fetches retrieve branch heads.

Provisioning MUST be idempotent: re-running against an already-provisioned project MUST NOT re-clone or error.

If a project has no `git_remote_url`, or cloning fails, or free disk space is below the configured threshold, the backend MUST mark that project unhealthy, record the reason, and continue provisioning the remaining projects without aborting the process.

#### Scenario: First provisioning creates bare and resident worktree

- **WHEN** a project with a reachable `git_remote_url` and `default_branches: [main]` is provisioned and `{repo_path}/.bare/` does not exist
- **THEN** `{repo_path}/.bare/` is created and `{repo_path}/main/` contains a worktree checked out to branch `main`

#### Scenario: Re-provisioning is idempotent

- **WHEN** a project is provisioned a second time and `{repo_path}/.bare/` already exists
- **THEN** no re-clone occurs and provisioning succeeds without error

#### Scenario: Missing remote URL isolates failure

- **WHEN** one project lacks `git_remote_url` and another project in the same load has a valid one
- **THEN** the first project is marked unhealthy with a recorded reason and the second project is still provisioned

#### Scenario: Insufficient disk space aborts the operation only

- **WHEN** free disk space is below the configured threshold before a clone or worktree add
- **THEN** the operation is refused, the project is marked unhealthy, and the process does not crash


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
### Requirement: Worktree paths are derived from branch names without collision

The backend SHALL derive a worktree directory name from a branch name by escaping every character outside `[A-Za-z0-9._-]` (including `/`) to `-`. For merge-request worktrees, the backend MUST append `-` followed by a short hash of the full branch name so that distinct branch names never map to the same directory. Resident (default-branch) worktrees MUST use the escaped name without a hash suffix.

Multiple merge requests that share the same source branch MUST map to the same merge-request worktree directory.

#### Scenario: Distinct branches with escape collision get distinct directories

- **WHEN** merge-request worktree names are derived for branches `feature/x` and `feature-x`
- **THEN** the two resulting directory names differ

##### Example: escape and hash rules

| Branch | Kind | Directory name shape | Notes |
| ------ | ---- | -------------------- | ----- |
| `main` | resident | `main` | escaped, no hash |
| `feature/x` | mr | `feature-x-<hash(feature/x)>` | `/` escaped, hash disambiguates |
| `feature-x` | mr | `feature-x-<hash(feature-x)>` | different hash from `feature/x` |
| `fix bug#1` | mr | `fix-bug-1-<hash(fix bug#1)>` | space and `#` escaped |


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
### Requirement: A worktree is supplied and updated on demand for a branch

The backend SHALL provide an operation that, given a project and a branch, returns the absolute path to that branch's worktree. If the worktree does not exist, the operation MUST create it from the bare repository. If it exists, the operation MUST run `git fetch origin <branch>` for that single ref and then `git reset --hard origin/<branch>` before returning the path.

On transient fetch failure, the operation MUST retry up to 3 times with exponential backoff; if all retries fail, it MUST return an error and leave the existing worktree unchanged.

If the fetch reports that the branch no longer exists on the remote, the operation MUST remove that worktree via `git worktree remove` and return an error indicating the branch is gone.

#### Scenario: Existing worktree is force-aligned to remote

- **WHEN** the supply operation is called for a branch whose worktree already exists and the remote branch was force-pushed
- **THEN** the worktree is fetched and hard-reset to `origin/<branch>` before the path is returned

#### Scenario: Transient fetch failure leaves worktree untouched

- **WHEN** every fetch attempt fails with a transient network error
- **THEN** the operation returns an error after 3 retries and the existing worktree content is unchanged

#### Scenario: Deleted remote branch removes the worktree

- **WHEN** the supply operation fetches a branch that no longer exists on the remote
- **THEN** the worktree is removed and the operation returns a branch-gone error


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
### Requirement: Worktree operations are serialized per repository

The backend SHALL serialize `git worktree add`, `fetch`, and `reset` operations that target the same bare repository, keyed by `repo_path`. Operations targeting different repositories MUST be allowed to run concurrently.

#### Scenario: Same repository operations do not interleave

- **WHEN** two supply operations target branches in the same repository concurrently
- **THEN** they execute one at a time under a per-repository lock

#### Scenario: Different repositories run concurrently

- **WHEN** supply operations target branches in two different repositories
- **THEN** they are allowed to proceed concurrently

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
### Requirement: Merge-request worktrees are provisioned on demand during a scan

When a scan needs a working directory for a specific merge request, the backend SHALL fetch the merge request's source branch from `origin` into the project's `.bare/` repository, then create the corresponding merge-request worktree if it does not already exist, using the directory naming rule already defined for merge-request worktrees (escaped branch name plus short hash suffix).

If a worktree for that source branch already exists, the backend MUST reuse it without re-fetching a full clone and MUST still perform the branch-scoped fetch to update it to the latest commit.

If the fetch fails (for example, the branch was deleted or the remote is unreachable), the backend MUST NOT create or update the worktree for that merge request and MUST record the failure without aborting the scan of other merge requests in the same project.

#### Scenario: First scan of a merge request creates its worktree

- **WHEN** a scan processes a merge request with source branch `feature/x` and no worktree for that branch exists yet
- **THEN** `origin` is fetched for `feature/x` and a worktree is created at the escaped-and-hashed directory name for `feature/x`

#### Scenario: Two merge requests sharing a source branch reuse one worktree

- **WHEN** two merge requests in the same project both have source branch `feature/x`
- **THEN** both scans resolve to the same worktree directory and no second worktree is created

#### Scenario: Unreachable source branch skips only that merge request

- **WHEN** a scan processes a merge request whose source branch was deleted on the remote
- **THEN** the fetch fails, no worktree is created for that merge request, the failure is recorded, and scanning continues for other merge requests in the same project

##### Example: one deleted branch among three merge requests

- **GIVEN** a project has three open merge requests with source branches `feature/a` (exists), `feature/b` (deleted on remote), and `feature/c` (exists)
- **WHEN** the scan runs `provision_mr_worktree` for all three in sequence
- **THEN** worktrees are created for `feature/a` and `feature/c`, `feature/b` records a failure and has no worktree, and the scan still processes `feature/c` after `feature/b` fails

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