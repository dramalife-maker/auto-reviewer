## ADDED Requirements

### Requirement: Projects load from YAML at startup

The backend SHALL load project definitions from a YAML file at repository root named `projects.yaml` unless overridden by environment variable `PROJECTS_CONFIG`.

Each entry MUST include `name` and `repo_path`. Optional field `git_remote_url` SHALL be stored when present.

On load, the server SHALL upsert rows in the `projects` table keyed by unique `name`.

#### Scenario: Load two projects from YAML

- **WHEN** `projects.yaml` contains two valid project entries
- **THEN** the `projects` table contains exactly two rows with matching `name` and `repo_path` values

### Requirement: Git repository detection updates project metadata

For each loaded project, the backend SHALL detect whether `repo_path` is a git working copy and store `is_git_repo` as 1 or 0 and `default_branch` when detectable.

#### Scenario: Valid git repository path

- **WHEN** `repo_path` points to a directory containing a `.git` folder with default branch `main`
- **THEN** the project row has `is_git_repo=1` and `default_branch='main'`

#### Scenario: Non-git path

- **WHEN** `repo_path` points to a directory that is not a git repository
- **THEN** the project row has `is_git_repo=0` and the project remains stored for later correction

