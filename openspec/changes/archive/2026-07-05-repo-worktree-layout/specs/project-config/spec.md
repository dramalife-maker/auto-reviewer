## MODIFIED Requirements

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

### Requirement: Git repository detection updates project metadata

For each loaded project, the backend SHALL provision the bare repository and resident worktrees (see the `repo-worktree` capability) and record whether provisioning succeeded. When the bare repository is present, the backend SHALL store `is_git_repo=1` and `default_branch` set to the first entry of `default_branches`. When provisioning fails or the project is unhealthy, the backend SHALL store `is_git_repo=0` and keep the project stored for later correction.

#### Scenario: Successful provisioning marks project as git repo

- **WHEN** a project's bare repository is provisioned and its resident worktrees are created
- **THEN** the project row has `is_git_repo=1` and `default_branch` equal to the first `default_branches` entry

#### Scenario: Failed provisioning keeps project for correction

- **WHEN** provisioning fails for a project (for example, unreachable remote)
- **THEN** the project row has `is_git_repo=0` and the project remains stored
