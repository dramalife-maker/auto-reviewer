## ADDED Requirements

### Requirement: Weekly batch manifest includes open pending items

Before spawning the reviewer-batch subprocess, the backend MUST include an `open_pending` array on the weekly `manifest.json` for that project.

Each element MUST contain: `id` (pending item id), `person_id`, `display_name` (canonical `people.display_name`), and `question`.

The array MUST contain every `pending_items` row for that `project_id` with `status='open'`, ordered stably by `person_id` ascending then `id` ascending.

When no open pending rows exist for the project, `open_pending` MUST be an empty array.

#### Scenario: Manifest lists open pending for the project

- **GIVEN** project G has an open pending item for person Alice with question `Why choose A?`
- **AND** project G has a resolved pending item with a different question
- **WHEN** the weekly manifest is written for project G
- **THEN** `open_pending` contains exactly one element for Alice with that question and its numeric `id`
- **AND** the resolved item is omitted

##### Example: open pending shape

- **GIVEN** open row `id=7`, `person_id=1`, `display_name="Alice Chen"`, `question="Why choose A?"`
- **WHEN** the weekly manifest is written
- **THEN** `open_pending` includes `{ "id": 7, "person_id": 1, "display_name": "Alice Chen", "question": "Why choose A?" }`

#### Scenario: Empty open pending when none exist

- **GIVEN** project G has no open pending items
- **WHEN** the weekly manifest is written for project G
- **THEN** `open_pending` is an empty array

### Requirement: Reviewer-batch reuses open pending question text verbatim

The reviewer-batch workflow MUST read `manifest.open_pending` when composing `## 待確認` for each author.

For each `open_pending` entry that matches the current author by `person_id` or `display_name`, the workflow MUST choose exactly one of the following for this run's `## 待確認`:

1. Include a bullet whose text is exactly equal to that entry's `question` (no paraphrase or rewording), or
2. Omit that question from `## 待確認` entirely when the issue is no longer relevant for this run.

Omitting an open question MUST NOT resolve it in the database (the worker and workflow do not write SQLite).

New questions that do not correspond to any `open_pending` entry for that author MUST be allowed as additional bullets under the usual 0–5 limit, using new wording only for genuinely new issues.

#### Scenario: Continuing open issue keeps exact wording

- **GIVEN** `open_pending` contains `{ "display_name": "Alice Chen", "question": "Why choose A?" }`
- **AND** the issue remains relevant this week
- **WHEN** the workflow writes Alice Chen's `summary.md`
- **THEN** `## 待確認` includes a bullet whose text is exactly `Why choose A?`

#### Scenario: Stale open issue omitted from summary

- **GIVEN** `open_pending` contains an open question for Alice Chen
- **AND** the workflow judges the issue no longer relevant this week
- **WHEN** the workflow writes Alice Chen's `summary.md`
- **THEN** that question is absent from `## 待確認`
- **AND** no database resolve is performed by the workflow
