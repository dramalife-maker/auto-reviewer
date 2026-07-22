## ADDED Requirements

### Requirement: Cancelled is a terminal run state

The backend SHALL treat `cancelled` as a terminal value for both `runs.status` and `run_projects.state`. A run or project in state `cancelled` MUST NOT be claimed, resumed, or re-executed.

#### Scenario: Cancelled run is not claimed for execution

- **WHEN** the run worker looks for queued work and a run's status is `cancelled`
- **THEN** no project belonging to that run is claimed

### Requirement: Run finalization preserves cancelled status

When the backend evaluates whether a run is complete, it MUST leave the run's status unchanged if that status is already `cancelled`. Projects that were still executing when the run was cancelled MUST NOT cause the run to be finalized as `success`, `partial`, or `failed`.

#### Scenario: Late-finishing project does not overwrite cancelled status

- **GIVEN** a run has status `cancelled` while one of its projects is still finishing
- **WHEN** that project completes and run finalization is evaluated
- **THEN** the run's status remains `cancelled`

##### Example: finalization outcomes by prior status

| run status before finalization | project outcomes | run status after |
| ------------------------------ | ---------------- | ---------------- |
| running | all succeeded | success |
| running | some skipped_timeout | partial |
| running | some failed, none skipped | failed |
| cancelled | any | cancelled |
