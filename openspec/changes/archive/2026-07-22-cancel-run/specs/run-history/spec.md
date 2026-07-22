## ADDED Requirements

### Requirement: Run history surfaces cancelled status

The execution history list and run detail views SHALL render `cancelled` as a distinct run status, visually separate from `failed`. Projects in state `cancelled` MUST be rendered as cancelled rather than as failures.

#### Scenario: Cancelled run is listed distinctly

- **WHEN** a user views the execution history list containing a run whose status is `cancelled`
- **THEN** that run is shown with a cancelled status distinct from `failed`

#### Scenario: Cancelled project is shown in run detail

- **WHEN** a user opens the detail view of a cancelled run
- **THEN** each project in state `cancelled` is shown as cancelled rather than as failed

### Requirement: Run history offers cancellation for in-progress runs

The run history UI SHALL offer a cancel action for any run whose status is `running`, and MUST NOT offer it for runs in a terminal status. Triggering the action MUST call the cancel run endpoint and reflect the resulting status without requiring a manual page reload.

#### Scenario: Cancel action is available on a running run

- **WHEN** a user views a run whose status is `running`
- **THEN** a cancel action is available for that run

#### Scenario: Cancel action is absent on a terminal run

- **WHEN** a user views a run whose status is `success`, `partial`, `failed`, or `cancelled`
- **THEN** no cancel action is offered for that run

#### Scenario: Triggering cancellation updates the displayed status

- **WHEN** a user triggers the cancel action on a running run and the request succeeds
- **THEN** the displayed run status becomes `cancelled` without a manual page reload
