## ADDED Requirements

### Requirement: Per-run cancellation tokens derive from the shutdown token

Each run's cancellation token MUST be derived from the process shutdown token, so that process shutdown continues to propagate to every executing project without a separate propagation path. Cancelling one run's token MUST NOT cancel the process shutdown token or any other run's token.

#### Scenario: Shutdown propagates through per-run tokens

- **GIVEN** a run is executing under its own cancellation token
- **WHEN** the process shutdown token is cancelled
- **THEN** that run's executing projects observe cancellation and are terminated

#### Scenario: Cancelling one run leaves shutdown and other runs intact

- **GIVEN** two runs are executing, each under its own cancellation token
- **WHEN** one run is cancelled by a user
- **THEN** the process shutdown token remains uncancelled and the other run continues executing

### Requirement: Cancellation source determines the terminal state

When an executing project observes cancellation, the backend MUST determine the source by inspecting the process shutdown token. If the shutdown token is cancelled, the project MUST be marked `failed` with an error containing the exact substring `interrupted by shutdown`. Otherwise the cancellation originated from a user and the project MUST be marked `cancelled`.

#### Scenario: Shutdown-sourced cancellation marks the project failed

- **WHEN** an executing project observes cancellation while the process shutdown token is cancelled
- **THEN** that project is marked `failed` with an error containing `interrupted by shutdown`

#### Scenario: User-sourced cancellation marks the project cancelled

- **WHEN** an executing project observes cancellation while the process shutdown token is not cancelled
- **THEN** that project is marked `cancelled`

##### Example: terminal state by cancellation source

| shutdown token cancelled | project terminal state | error contains |
| ------------------------ | ---------------------- | -------------- |
| yes | failed | interrupted by shutdown |
| no | cancelled | (no shutdown error) |

### Requirement: Startup recovery leaves cancelled rows untouched

Startup recovery of orphaned rows MUST NOT alter `run_projects` rows in state `cancelled`, because `cancelled` is a terminal state rather than an interrupted one.

#### Scenario: Cancelled row survives a process restart

- **GIVEN** the database contains a `run_projects` row in state `cancelled` from a previous process
- **WHEN** the server starts and runs orphan recovery
- **THEN** that row remains `cancelled` and its error message is unchanged

### Requirement: Run cancellation tokens are released when a run ends

The backend MUST remove a run's cancellation token from its registry once the run reaches a terminal status, whether it ended by cancellation or by normal completion, so that a long-lived process does not accumulate tokens.

#### Scenario: Token is released after normal completion

- **WHEN** a run reaches a terminal status without being cancelled
- **THEN** its cancellation token is no longer retained in the registry

#### Scenario: Token is released after cancellation

- **WHEN** a run reaches terminal status `cancelled`
- **THEN** its cancellation token is no longer retained in the registry
