# graceful-shutdown Specification

## Purpose

TBD - created by archiving change 'graceful-shutdown'. Update Purpose after archive.

## Requirements

### Requirement: Process shutdown is triggered by Ctrl+C and Unix SIGTERM

The backend process SHALL treat Ctrl+C and, on Unix platforms, SIGTERM as the same shutdown signal. On receipt, the process MUST begin the coordinated shutdown sequence defined by this capability. Windows builds MUST honor Ctrl+C and MUST NOT require SIGTERM.

#### Scenario: Ctrl+C starts shutdown

- **WHEN** the running server receives Ctrl+C
- **THEN** the coordinated shutdown sequence begins

#### Scenario: SIGTERM starts shutdown on Unix

- **WHEN** a Unix build receives SIGTERM
- **THEN** the coordinated shutdown sequence begins identically to Ctrl+C


<!-- @trace
source: graceful-shutdown
updated: 2026-07-17
code:
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/reports.rs
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/worker.rs
  - frontend/src/types.ts
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: Coordinated shutdown stops HTTP, scheduler, and new worker jobs

During coordinated shutdown the backend MUST stop accepting new HTTP connections, MUST stop the cron scheduler from enqueueing new runs, and MUST stop the run worker from dequeuing additional `queued` `run_projects`. In-flight HTTP requests MUST be allowed to finish or fail quickly after cancellation. The process MUST exit within 15 seconds of the shutdown signal; if cleanup has not finished by then the process MUST terminate anyway.

#### Scenario: Shutdown stops new work within the deadline

- **WHEN** shutdown begins while the worker and scheduler are active
- **THEN** no new cron-triggered runs are enqueued, no additional `queued` projects are dequeued after cancellation is observed, new HTTP connections are not accepted, and the process exits within 15 seconds


<!-- @trace
source: graceful-shutdown
updated: 2026-07-17
code:
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/reports.rs
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/worker.rs
  - frontend/src/types.ts
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: In-flight reviewer subprocesses are cancelled and killed

When shutdown cancellation is observed, every in-flight reviewer subprocess (weekly batch, MR scan, and HTTP agent-turn) MUST be terminated using process-tree kill semantics already used for timeout (including Windows `taskkill /F /T` when applicable). Cancellation MUST NOT be recorded as a timeout skip.

#### Scenario: Cancelled subprocess is killed and not marked timeout

- **WHEN** a reviewer subprocess is waiting and the shutdown cancellation token fires
- **THEN** the process tree is killed and the outcome is failure due to shutdown, not `skipped_timeout`


<!-- @trace
source: graceful-shutdown
updated: 2026-07-17
code:
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/reports.rs
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/worker.rs
  - frontend/src/types.ts
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: Interrupted running projects are marked failed

For each `run_projects` row that is `running` when interrupted by shutdown, the backend MUST set `state='failed'` with an error message that contains the exact substring `interrupted by shutdown`, and MUST finalize the parent run when no `queued` or `running` projects remain. Rows that are still `queued` at shutdown MUST remain `queued` so a subsequent process start can dequeue them.

#### Scenario: Running project becomes failed on shutdown

- **WHEN** a `run_projects` row is `running` and shutdown cancels its subprocess
- **THEN** that row ends as `failed` with error containing `interrupted by shutdown` and the parent run is finalized once no pending projects remain

#### Scenario: Queued projects survive shutdown

- **WHEN** shutdown occurs while some `run_projects` rows are `queued` and none remain `running`
- **THEN** those `queued` rows stay `queued`

##### Example: mixed states at shutdown

| state before | state after shutdown | error contains |
| ------------ | -------------------- | -------------- |
| running | failed | interrupted by shutdown |
| queued | queued | (unchanged / null) |


<!-- @trace
source: graceful-shutdown
updated: 2026-07-17
code:
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/reports.rs
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/worker.rs
  - frontend/src/types.ts
tests:
  - backend/tests/report_reader.rs
-->

---
### Requirement: Startup recovers orphaned running projects

Before the run worker starts accepting work, startup MUST find every `run_projects` row with `state='running'`, set each to `failed` with an error message that contains the exact substring `interrupted by previous shutdown`, and finalize each affected parent run when appropriate. This recovery MUST run even when no shutdown signal was observed in the current process (for example after a forced kill).

#### Scenario: Orphaned running row is failed on next start

- **WHEN** the database contains a `run_projects` row with `state='running'` from a previous process and the server starts
- **THEN** that row becomes `failed` with error containing `interrupted by previous shutdown` before the worker begins dequeuing, and the parent run does not remain forever `running` solely because of that orphan

<!-- @trace
source: graceful-shutdown
updated: 2026-07-17
code:
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/reports.rs
  - backend/src/mr_change_materials.rs
  - backend/src/server.rs
  - backend/src/worker.rs
  - frontend/src/types.ts
tests:
  - backend/tests/report_reader.rs
-->

---
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


<!-- @trace
source: cancel-run
updated: 2026-07-22
code:
  - frontend/src/lib/format.ts
  - backend/src/runs.rs
  - backend/src/worker.rs
  - frontend/src/api.ts
  - backend/src/server.rs
  - frontend/src/pages/RunsPage.tsx
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/run_cancellation.rs
  - frontend/src/pages/RunsPage.test.tsx
-->

---
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


<!-- @trace
source: cancel-run
updated: 2026-07-22
code:
  - frontend/src/lib/format.ts
  - backend/src/runs.rs
  - backend/src/worker.rs
  - frontend/src/api.ts
  - backend/src/server.rs
  - frontend/src/pages/RunsPage.tsx
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/run_cancellation.rs
  - frontend/src/pages/RunsPage.test.tsx
-->

---
### Requirement: Startup recovery leaves cancelled rows untouched

Startup recovery of orphaned rows MUST NOT alter `run_projects` rows in state `cancelled`, because `cancelled` is a terminal state rather than an interrupted one.

#### Scenario: Cancelled row survives a process restart

- **GIVEN** the database contains a `run_projects` row in state `cancelled` from a previous process
- **WHEN** the server starts and runs orphan recovery
- **THEN** that row remains `cancelled` and its error message is unchanged


<!-- @trace
source: cancel-run
updated: 2026-07-22
code:
  - frontend/src/lib/format.ts
  - backend/src/runs.rs
  - backend/src/worker.rs
  - frontend/src/api.ts
  - backend/src/server.rs
  - frontend/src/pages/RunsPage.tsx
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/run_cancellation.rs
  - frontend/src/pages/RunsPage.test.tsx
-->

---
### Requirement: Run cancellation tokens are released when a run ends

The backend MUST remove a run's cancellation token from its registry once the run reaches a terminal status, whether it ended by cancellation or by normal completion, so that a long-lived process does not accumulate tokens.

#### Scenario: Token is released after normal completion

- **WHEN** a run reaches a terminal status without being cancelled
- **THEN** its cancellation token is no longer retained in the registry

#### Scenario: Token is released after cancellation

- **WHEN** a run reaches terminal status `cancelled`
- **THEN** its cancellation token is no longer retained in the registry

<!-- @trace
source: cancel-run
updated: 2026-07-22
code:
  - frontend/src/lib/format.ts
  - backend/src/runs.rs
  - backend/src/worker.rs
  - frontend/src/api.ts
  - backend/src/server.rs
  - frontend/src/pages/RunsPage.tsx
tests:
  - backend/tests/runs_execution.rs
  - backend/tests/run_cancellation.rs
  - frontend/src/pages/RunsPage.test.tsx
-->