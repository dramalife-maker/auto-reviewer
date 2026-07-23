# review-ignore-globs Specification

## Purpose

TBD - created by archiving change 'review-ignore-globs'. Update Purpose after archive.

## Requirements

### Requirement: Global review ignore list is persisted in the database

The backend SHALL store a single global review ignore list in a dedicated single-row settings table keyed by `id = 1`. The list SHALL be persisted as a JSON string array of raw git pathspec patterns, and SHALL default to an empty array. The migration that introduces the table SHALL insert the `id = 1` row so that reads never depend on lazy initialization, and SHALL record its schema version like every other migration in the project.

The stored values SHALL be raw patterns without any git pathspec magic prefix; the prefix SHALL be applied only when a pattern is passed to git.

#### Scenario: Fresh database yields an empty list

- **WHEN** the backend starts against a database that has just applied the migration
- **THEN** reading the review settings returns an `ignore_globs` array with zero entries

#### Scenario: Stored list survives restart

- **WHEN** the ignore list is updated and the backend process restarts
- **THEN** reading the review settings returns the previously stored entries


<!-- @trace
source: review-ignore-globs
updated: 2026-07-23
code:
  - backend/src/summary.rs
  - frontend/src/api.ts
  - skills/scan-mrs-headless/WORKFLOW.md
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/review_settings.rs
  - backend/src/error.rs
  - backend/src/report_chat.rs
  - backend/migrations/017_review_settings.sql
  - backend/src/executor.rs
  - backend/migrations/016_people_folder_name.sql
  - backend/src/person_trends.rs
  - backend/src/mr_reviews.rs
  - backend/src/lib.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - frontend/src/pages/DashboardPage.tsx
  - backend/src/identity.rs
  - backend/src/runs.rs
  - backend/src/mr_change_materials.rs
  - backend/src/worker.rs
tests:
  - backend/tests/identity.rs
  - frontend/src/pages/DashboardPage.reviewSettings.test.tsx
  - backend/tests/review_settings.rs
  - frontend/src/pages/DashboardPage.catchup.test.tsx
-->

---
### Requirement: Review settings API reads and replaces the ignore list

The backend SHALL expose `GET /api/review-settings` returning a JSON object with an `ignore_globs` string array, and `PUT /api/review-settings` accepting an object of the same shape. `PUT` SHALL apply full-replacement semantics: the submitted list replaces the stored list in its entirety, and the response body SHALL contain the normalized result that was stored.

#### Scenario: Update replaces the entire list

- **WHEN** the stored list is `["*.lock"]` and a client sends `PUT` with `["vendor/**"]`
- **THEN** the stored list becomes `["vendor/**"]` and the response body contains exactly that list

#### Scenario: Read returns the stored list

- **WHEN** a client sends `GET` after a successful update
- **THEN** the response contains the same normalized entries the update returned


<!-- @trace
source: review-ignore-globs
updated: 2026-07-23
code:
  - backend/src/summary.rs
  - frontend/src/api.ts
  - skills/scan-mrs-headless/WORKFLOW.md
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/review_settings.rs
  - backend/src/error.rs
  - backend/src/report_chat.rs
  - backend/migrations/017_review_settings.sql
  - backend/src/executor.rs
  - backend/migrations/016_people_folder_name.sql
  - backend/src/person_trends.rs
  - backend/src/mr_reviews.rs
  - backend/src/lib.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - frontend/src/pages/DashboardPage.tsx
  - backend/src/identity.rs
  - backend/src/runs.rs
  - backend/src/mr_change_materials.rs
  - backend/src/worker.rs
tests:
  - backend/tests/identity.rs
  - frontend/src/pages/DashboardPage.reviewSettings.test.tsx
  - backend/tests/review_settings.rs
  - frontend/src/pages/DashboardPage.catchup.test.tsx
-->

---
### Requirement: Ignore list entries are normalized and validated on write

On `PUT`, the backend SHALL normalize the submitted entries by trimming surrounding whitespace, discarding entries that are empty after trimming, and removing duplicates while preserving the first occurrence order. Normalization SHALL NOT be treated as an error.

The backend SHALL reject the request with HTTP 400 when any entry begins with a colon, when any entry exceeds 200 characters after trimming, or when the list contains more than 100 entries after normalization. The error body SHALL identify which rule was violated. Validation SHALL be enforced by the backend; the frontend MUST NOT be the only place these rules are applied.

#### Scenario: Normalization is applied silently

- **WHEN** a client submits entries requiring trimming, deduplication, or removal of blanks
- **THEN** the request succeeds and the response contains only the normalized entries

##### Example: normalization cases

| Submitted | Stored | Notes |
| --------- | ------ | ----- |
| `["  *.lock  "]` | `["*.lock"]` | surrounding whitespace trimmed |
| `["*.lock", "*.lock"]` | `["*.lock"]` | duplicate dropped, first occurrence kept |
| `["*.lock", "", "   "]` | `["*.lock"]` | blank entries discarded |
| `["b.lock", "a.lock"]` | `["b.lock", "a.lock"]` | order preserved, not sorted |

#### Scenario: Invalid entries are rejected

- **WHEN** a client submits an entry that violates a validation rule
- **THEN** the backend responds with HTTP 400, the stored list is left unchanged, and the message names the violated rule

##### Example: rejection cases

| Submitted | Result | Notes |
| --------- | ------ | ----- |
| `[":(exclude)*.lock"]` | 400 | leading colon would produce a doubled pathspec magic prefix |
| `[":(top)"]` | 400 | leading colon |
| one entry of 201 characters | 400 | exceeds the per-entry length limit |
| 101 distinct entries | 400 | exceeds the list size limit |


<!-- @trace
source: review-ignore-globs
updated: 2026-07-23
code:
  - backend/src/summary.rs
  - frontend/src/api.ts
  - skills/scan-mrs-headless/WORKFLOW.md
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/review_settings.rs
  - backend/src/error.rs
  - backend/src/report_chat.rs
  - backend/migrations/017_review_settings.sql
  - backend/src/executor.rs
  - backend/migrations/016_people_folder_name.sql
  - backend/src/person_trends.rs
  - backend/src/mr_reviews.rs
  - backend/src/lib.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - frontend/src/pages/DashboardPage.tsx
  - backend/src/identity.rs
  - backend/src/runs.rs
  - backend/src/mr_change_materials.rs
  - backend/src/worker.rs
tests:
  - backend/tests/identity.rs
  - frontend/src/pages/DashboardPage.reviewSettings.test.tsx
  - backend/tests/review_settings.rs
  - frontend/src/pages/DashboardPage.catchup.test.tsx
-->

---
### Requirement: Run manifests expose the ignore list to agents

Both the weekly run manifest and the MR poll manifest SHALL include an `ignore_globs` string array carrying the stored list at the time the manifest is written. The field SHALL be omitted from the serialized manifest when the list is empty.

The `reviewer-batch` and `scan-mrs-headless` workflow documents SHALL instruct the agent that any git command it runs itself MUST append exclusion pathspecs built from the manifest's `ignore_globs`. This instruction is advisory in effect: the agent retains an unrestricted shell, so the backend MUST NOT depend on it for correctness.

#### Scenario: Manifest carries the configured list

- **WHEN** the ignore list is non-empty and a run manifest is written
- **THEN** the manifest JSON contains `ignore_globs` with the stored entries

#### Scenario: Empty list is omitted from the manifest

- **WHEN** the ignore list is empty and a run manifest is written
- **THEN** the manifest JSON contains no `ignore_globs` key


<!-- @trace
source: review-ignore-globs
updated: 2026-07-23
code:
  - backend/src/summary.rs
  - frontend/src/api.ts
  - skills/scan-mrs-headless/WORKFLOW.md
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/review_settings.rs
  - backend/src/error.rs
  - backend/src/report_chat.rs
  - backend/migrations/017_review_settings.sql
  - backend/src/executor.rs
  - backend/migrations/016_people_folder_name.sql
  - backend/src/person_trends.rs
  - backend/src/mr_reviews.rs
  - backend/src/lib.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - frontend/src/pages/DashboardPage.tsx
  - backend/src/identity.rs
  - backend/src/runs.rs
  - backend/src/mr_change_materials.rs
  - backend/src/worker.rs
tests:
  - backend/tests/identity.rs
  - frontend/src/pages/DashboardPage.reviewSettings.test.tsx
  - backend/tests/review_settings.rs
  - frontend/src/pages/DashboardPage.catchup.test.tsx
-->

---
### Requirement: Dashboard exposes the ignore list for editing

The frontend SHALL present the ignore list in a dedicated card on the Dashboard, separate from the schedule card and with its own save action, because it targets a different endpoint. The editor SHALL accept one pattern per line and SHALL start empty for a fresh installation, with placeholder text illustrating common patterns.

After a successful save the UI SHALL inform the operator that the change takes effect on the next run and that no service restart is required.

#### Scenario: Operator saves a list

- **WHEN** the operator enters patterns one per line and triggers save
- **THEN** the frontend calls the update endpoint with those patterns and, on success, shows a confirmation stating the change applies to the next run

#### Scenario: Rejected save surfaces the error

- **WHEN** the update endpoint responds with HTTP 400
- **THEN** the frontend surfaces the returned message and does not report the save as successful

<!-- @trace
source: review-ignore-globs
updated: 2026-07-23
code:
  - backend/src/summary.rs
  - frontend/src/api.ts
  - skills/scan-mrs-headless/WORKFLOW.md
  - backend/src/server.rs
  - skills/reviewer-batch/WORKFLOW.md
  - backend/src/review_settings.rs
  - backend/src/error.rs
  - backend/src/report_chat.rs
  - backend/migrations/017_review_settings.sql
  - backend/src/executor.rs
  - backend/migrations/016_people_folder_name.sql
  - backend/src/person_trends.rs
  - backend/src/mr_reviews.rs
  - backend/src/lib.rs
  - backend/src/reports.rs
  - skills/reviewer-batch/output-contract.md
  - frontend/src/types.ts
  - backend/src/pending_items.rs
  - frontend/src/pages/DashboardPage.tsx
  - backend/src/identity.rs
  - backend/src/runs.rs
  - backend/src/mr_change_materials.rs
  - backend/src/worker.rs
tests:
  - backend/tests/identity.rs
  - frontend/src/pages/DashboardPage.reviewSettings.test.tsx
  - backend/tests/review_settings.rs
  - frontend/src/pages/DashboardPage.catchup.test.tsx
-->