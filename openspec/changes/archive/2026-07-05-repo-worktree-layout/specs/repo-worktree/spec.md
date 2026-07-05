## ADDED Requirements

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

### Requirement: Worktree operations are serialized per repository

The backend SHALL serialize `git worktree add`, `fetch`, and `reset` operations that target the same bare repository, keyed by `repo_path`. Operations targeting different repositories MUST be allowed to run concurrently.

#### Scenario: Same repository operations do not interleave

- **WHEN** two supply operations target branches in the same repository concurrently
- **THEN** they execute one at a time under a per-repository lock

#### Scenario: Different repositories run concurrently

- **WHEN** supply operations target branches in two different repositories
- **THEN** they are allowed to proceed concurrently
