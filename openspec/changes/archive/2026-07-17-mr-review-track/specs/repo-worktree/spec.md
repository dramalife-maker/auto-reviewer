## ADDED Requirements

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
