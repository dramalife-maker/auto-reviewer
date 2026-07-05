# project-config Specification

## Purpose

TBD - created by archiving change 'cloud-reviewer-mvp'. Update Purpose after archive.

## Requirements

### Requirement: Projects load from YAML at startup

The backend SHALL load project definitions from a YAML file at repository root named `projects.yaml` unless overridden by environment variable `PROJECTS_CONFIG`.

Each entry MUST include `name` and `repo_path`. Optional field `git_remote_url` SHALL be stored when present.

Before upserting, the server SHALL resolve each entry's `repo_path` using `DATA_ROOT_DIR`:

- If `repo_path` is an absolute filesystem path, the server MUST store it unchanged.
- If `repo_path` is a relative path whose first component is `.` or `..`, the server MUST treat it as an explicit path relative to the process current working directory and store it unchanged.
- Otherwise, the server MUST resolve `repo_path` as `{DATA_ROOT_DIR}/repos/{repo_path}` and store the resolved path.

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


<!-- @trace
source: repo-path-repos-prefix
updated: 2026-07-05
code:
  - README.md
  - backend/src/projects.rs
  - projects.yaml
  - backend/src/lib.rs
tests:
  - backend/tests/project_config.rs
-->

---
### Requirement: Git repository detection updates project metadata

For each loaded project, the backend SHALL detect whether `repo_path` is a git working copy and store `is_git_repo` as 1 or 0 and `default_branch` when detectable.

#### Scenario: Valid git repository path

- **WHEN** `repo_path` points to a directory containing a `.git` folder with default branch `main`
- **THEN** the project row has `is_git_repo=1` and `default_branch='main'`

#### Scenario: Non-git path

- **WHEN** `repo_path` points to a directory that is not a git repository
- **THEN** the project row has `is_git_repo=0` and the project remains stored for later correction

<!-- @trace
source: cloud-reviewer-mvp
updated: 2026-07-05
code:
  - README.md
  - backend/src/main.rs
  - frontend/src/main.ts
  - backend/src/server.rs
  - backend/src/runs.rs
  - crates/app-env/Cargo.toml
  - backend/src/projects.rs
  - frontend/src/types.ts
  - frontend/src/app.ts
  - backend/Cargo.toml
  - docs/idea/schema.md
  - Cargo.toml
  - frontend/src/api.ts
  - backend/migrations/001_initial.sql
  - .env.example
  - frontend/index.html
  - backend/src/state.rs
  - frontend/public/favicon.svg
  - crates/app-env/src/lib.rs
  - docs/idea/spec.md
  - backend/src/reports.rs
  - backend/src/schedule.rs
  - frontend/src/assets/typescript.svg
  - frontend/src/assets/hero.png
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/error.rs
  - backend/src/lib.rs
  - frontend/src/assets/vite.svg
  - frontend/src/style.css
  - frontend/vite.config.ts
  - backend/src/executor.rs
  - frontend/package.json
  - backend/src/summary.rs
  - projects.yaml
  - frontend/public/icons.svg
  - backend/src/db.rs
  - backend/src/worker.rs
  - frontend/tsconfig.json
  - skills/reviewer-batch/output-contract.md
  - backend/src/config.rs
tests:
  - backend/tests/fixtures/slow_executor.cmd
  - backend/tests/runs_execution.rs
  - backend/tests/scheduling.rs
  - backend/tests/project_config.rs
  - backend/tests/foundation.rs
  - backend/tests/report_reader.rs
-->