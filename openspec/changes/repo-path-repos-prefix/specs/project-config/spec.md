## MODIFIED Requirements

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
