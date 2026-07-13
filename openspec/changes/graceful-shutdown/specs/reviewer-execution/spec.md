## ADDED Requirements

### Requirement: Reviewer executors honor shutdown cancellation

Weekly batch, MR scan, and agent-turn executor paths MUST accept a shared cancellation token (or equivalent cooperative cancel signal from process shutdown). While waiting on a reviewer subprocess, the executor MUST race the wait against cancellation. If cancellation wins, the executor MUST kill the subprocess process tree using the same kill semantics as timeout handling, and MUST return a failure outcome whose error identifies shutdown interruption (not a timeout skip).

HTTP agent-turn handlers MUST use the same cancellation token from application state so an in-flight clarification turn is cancelled during process shutdown.

#### Scenario: Weekly or MR executor fails on cancel

- **WHEN** `execute_weekly_batch` or `execute_mr_review` is waiting on a child and the shutdown cancellation token fires
- **THEN** the child process tree is killed and the function returns a failed outcome (not `skipped_timeout`) with an error identifying shutdown interruption

#### Scenario: Agent-turn honors the same token

- **WHEN** an HTTP agent-turn is waiting on a child and process shutdown cancels the shared token
- **THEN** the child process tree is killed and the turn fails without leaving an orphaned reviewer process
