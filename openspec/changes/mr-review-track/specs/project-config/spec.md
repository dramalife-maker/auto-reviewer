## ADDED Requirements

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
