## ADDED Requirements

### Requirement: Weekly and MR manifests include notes_dir

Before spawning a weekly-batch or MR-poll reviewer subprocess, the backend MUST include `notes_dir` on that project’s `manifest.json`.

`notes_dir` MUST be the absolute (or data-root-normalized) path `{DATA_ROOT_DIR}/reports/{project_name}/.notes`, using the same path separator normalization as `report_root`.

The backend MUST NOT require the directory to exist when writing the manifest. Creating `.notes/` remains the writer’s responsibility on first ADR write.

#### Scenario: Weekly manifest exposes notes_dir

- **WHEN** a weekly batch run prepares a manifest for project `game-backend`
- **THEN** `manifest.json` contains `notes_dir` ending with `reports/game-backend/.notes` (after normalization)

#### Scenario: MR poll manifest exposes notes_dir

- **WHEN** an MR poll run prepares a manifest for project `game-backend`
- **THEN** `manifest.json` contains the same `notes_dir` value shape as the weekly manifest for that project

### Requirement: Agent-turn receives ADR skill and notes_dir

When the backend executes `POST /api/mr-reviews/:id/agent-turn` against a draft review with a resumable agent session, it MUST supply the project ADR skill materials from `skills/project-adr-notes/` (Claude: append-system-prompt-file equivalent; Cursor: equivalent prompt inclusion) and MUST expose the project `notes_dir` path in the turn context so Write/Read under that directory is possible.

Agent-turn ADR writes MUST NOT publish to GitLab and MUST NOT be required to modify the draft body. Draft re-read behavior after a turn remains unchanged from existing agent-turn draft handling.

#### Scenario: Agent-turn command includes ADR skill

- **WHEN** the executor builds an agent-turn command for Claude (non-stub executor)
- **THEN** the command includes the project-adr-notes skill file among appended system prompt files

#### Scenario: Turn context names notes_dir

- **WHEN** an agent-turn runs for a review belonging to project `game-backend`
- **THEN** the turn context includes `notes_dir` pointing at that project’s `.notes` directory

### Requirement: Reviewer workflows consume notes_dir

The `reviewer-batch` and `scan-mrs-headless` workflows MUST document and require: read `manifest.notes_dir` / `index.md` before adding technical-choice questions; write only under paths already allowed by their contracts plus `.notes` only when an interactive agent-turn skill applies (headless weekly/MR scan MUST NOT write ADRs in this change).

#### Scenario: Weekly workflow mentions notes_dir

- **WHEN** an implementer inspects `skills/reviewer-batch/WORKFLOW.md`
- **THEN** the workflow states that `manifest.notes_dir` MUST be read before composing new technical-choice `## 待確認` items

#### Scenario: MR scan workflow mentions notes_dir

- **WHEN** an implementer inspects `skills/scan-mrs-headless/WORKFLOW.md`
- **THEN** the workflow states that `manifest.notes_dir` MUST be read before composing suggested technical-choice follow-ups

