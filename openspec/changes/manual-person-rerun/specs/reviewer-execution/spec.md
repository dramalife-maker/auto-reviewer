## ADDED Requirements

### Requirement: Manual single-person run enqueues one project scoped to one person

The backend SHALL expose `POST /api/runs` accepting JSON `{ "trigger": "manual_person", "project_name": <string>, "person_id": <integer> }`.

The handler MUST reject the request with HTTP 400 when `project_name` is missing or empty, or when `person_id` is missing.

The handler MUST resolve `project_name` to a `projects` row and return HTTP 404 when no such project exists. The handler MUST resolve `person_id` to a `people` row and return HTTP 404 when no such person exists. The handler MUST NOT reject the request based on whether the person has any commits, identities, or activity in the project.

When validation passes, the handler MUST create a `runs` row with `trigger='manual_person'`, `status='running'`, and `project_total=1`, and insert exactly one `run_projects` row for the resolved project with `state='queued'` and `person_id` set to the requested person.

The concurrency gate MUST be the same whole-system gate used by `manual_all` and `manual_project`: if any `run_projects` row is in state `queued` or `running` under an active run, the server MUST reject the new run with HTTP 409.

The response on success MUST be HTTP 201 with body `{ "run_id": <i64> }`.

#### Scenario: Start manual single-person run

- **WHEN** a client posts `{ "trigger": "manual_person", "project_name": "crm", "person_id": 1 }` and no run is active
- **THEN** the response is HTTP 201 with a run id, one `run_projects` row exists for the crm project with `state='queued'` and `person_id=1`

#### Scenario: Missing person_id is rejected

- **WHEN** a client posts `{ "trigger": "manual_person", "project_name": "crm" }` without `person_id`
- **THEN** the server responds HTTP 400 and creates no run

#### Scenario: Unknown person is rejected

- **WHEN** a client posts `manual_person` for an existing project but a `person_id` with no matching `people` row
- **THEN** the server responds HTTP 404 and creates no run

#### Scenario: Concurrency gate blocks single-person run

- **WHEN** any project already has a `run_projects` row in `queued` or `running` under an active run
- **AND** a client posts a `manual_person` run
- **THEN** the server responds HTTP 409 and creates no run

### Requirement: run_projects carries an optional person scope

The `run_projects` table SHALL include a nullable `person_id` column referencing `people(id)`. A NULL `person_id` MUST mean the run project processes all resolved authors (the existing batch behavior for `manual_all`, `manual_project`, MR, scheduled, and poll runs). A non-NULL `person_id` MUST mean the run project is scoped to that single person.

When the worker claims a queued `run_projects` row, the claimed row data MUST include its `person_id` so downstream weekly manifest generation can honor the scope.

#### Scenario: Batch run projects have null person scope

- **WHEN** a `manual_all` or `manual_project` run is created
- **THEN** every inserted `run_projects` row has `person_id` NULL

#### Scenario: Claimed run project exposes person scope

- **WHEN** the worker claims a queued `run_projects` row created by a `manual_person` run for person 1
- **THEN** the claimed row data reports `person_id = 1`

### Requirement: Weekly manifest is filtered to the run project person scope

When a weekly batch `run_projects` row has a non-NULL `person_id`, the backend SHALL filter the generated `manifest.json` so that `authors`, `open_pending`, and `published_pending_snippets` each contain only entries belonging to that person. Authors and open pending entries MUST be filtered by matching `person_id`. Published pending snippet paths MUST be filtered to those whose leading path segment equals that person's `people.display_name`.

When `person_id` is NULL, the manifest MUST include all resolved authors, all project open pending items, and all published pending snippets, unchanged from the existing batch behavior.

A person scope that matches no authors in the analysis window MUST NOT be an error: the manifest MUST be written with an empty `authors` array and the run MUST complete normally.

#### Scenario: Person-scoped manifest lists only the target person

- **GIVEN** a project with commits, open pending items, and published snippets for both person 1 ("Alice Chen") and person 2 ("Bob")
- **WHEN** the weekly manifest is written for a `run_projects` row with `person_id=1`
- **THEN** `authors` contains only Alice's entry, `open_pending` contains only Alice's items, and `published_pending_snippets` contains only paths under `Alice Chen/`

#### Scenario: Null person scope preserves full manifest

- **WHEN** the weekly manifest is written for a `run_projects` row with NULL `person_id`
- **THEN** `authors`, `open_pending`, and `published_pending_snippets` include every resolved author, open item, and published snippet for the project

#### Scenario: Person scope with no window activity completes as no-op

- **WHEN** the weekly manifest is written for a `person_id` whose person has no commits in the analysis window
- **THEN** the manifest is written with an empty `authors` array and the run project completes without error
