## ADDED Requirements

### Requirement: Cancel run API endpoint

The backend SHALL expose `POST /api/runs/{id}/cancel`, which cancels every project belonging to the identified run.

On success the endpoint MUST return `200` with the run's post-cancellation state, using the same response shape as the existing run detail endpoint. When no run exists with the given id the endpoint MUST return `404`. When the run has already reached a terminal status (`success`, `partial`, `failed`, or `cancelled`) the endpoint MUST return `409` and MUST NOT modify any row.

#### Scenario: Cancelling a running run succeeds

- **WHEN** a client sends `POST /api/runs/{id}/cancel` for a run whose status is `running`
- **THEN** the response is `200` and the run's status is `cancelled`

#### Scenario: Cancelling an unknown run is rejected

- **WHEN** a client sends `POST /api/runs/{id}/cancel` for an id that matches no run
- **THEN** the response is `404`

#### Scenario: Cancelling an already terminal run is rejected

- **WHEN** a client sends `POST /api/runs/{id}/cancel` for a run whose status is already terminal
- **THEN** the response is `409` and no `run_projects` row changes state

##### Example: terminal statuses rejected

| run status | response | run_projects modified |
| ---------- | -------- | --------------------- |
| running | 200 | yes |
| success | 409 | no |
| partial | 409 | no |
| failed | 409 | no |
| cancelled | 409 | no |

### Requirement: Cancellation terminates in-flight project work

Cancelling a run MUST terminate the agent subprocess of every project in that run whose state is `running`, without waiting for the per-project timeout to expire. Each such project MUST end in state `cancelled`.

#### Scenario: Running project is terminated and marked cancelled

- **WHEN** a run with a `running` project is cancelled
- **THEN** that project's agent subprocess is terminated and the project ends in state `cancelled`

### Requirement: Cancellation prevents queued projects from starting

Cancelling a run MUST set every `queued` project of that run to state `cancelled`, and those projects MUST NOT subsequently be claimed for execution.

#### Scenario: Queued project is cancelled without ever executing

- **WHEN** a run holding both a `running` and a `queued` project is cancelled
- **THEN** both projects end in state `cancelled` and the queued one is never claimed for execution

##### Example: mixed states at cancellation

| state before cancel | state after cancel | subprocess started |
| ------------------- | ------------------ | ------------------ |
| running | cancelled | yes, then terminated |
| queued | cancelled | no |

### Requirement: Cancellation is scoped to one run

Cancelling a run MUST NOT affect projects belonging to any other run. Concurrently executing runs MUST continue to completion.

#### Scenario: Concurrent run is unaffected

- **GIVEN** two runs are executing concurrently
- **WHEN** one of them is cancelled
- **THEN** the other run's projects continue executing and reach their normal terminal states

### Requirement: Cancellation preserves and ingests produced outputs

Cancelling a run MUST NOT delete outputs already written to disk, and the backend MUST ingest those outputs as it does for projects that end via the per-project timeout path.

#### Scenario: Output on disk survives cancellation and is ingested

- **GIVEN** a project has written output to disk before cancellation
- **WHEN** the run is cancelled
- **THEN** the output file remains on disk and is ingested into the database

#### Scenario: Ingest failure does not block cancellation

- **WHEN** ingest fails while cancelling a run
- **THEN** the failure is logged and the run still reaches status `cancelled`

### Requirement: User cancellation is distinguishable from process shutdown

The backend MUST record user-initiated cancellation and shutdown interruption as distinct outcomes. Projects ended by user cancellation MUST have state `cancelled`. Projects ended by process shutdown MUST retain state `failed` with an error containing the exact substring `interrupted by shutdown`.

#### Scenario: User cancellation yields cancelled state

- **WHEN** a running project ends because a user cancelled its run
- **THEN** that project's state is `cancelled`

#### Scenario: Shutdown interruption still yields failed state

- **WHEN** a running project ends because the process began shutdown
- **THEN** that project's state is `failed` with an error containing `interrupted by shutdown`
