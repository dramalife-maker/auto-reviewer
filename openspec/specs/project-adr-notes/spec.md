# project-adr-notes Specification

## Purpose

Project-level architecture decision records live under `reports/{project}/.notes/`, with an index-and-body contract (tl;dr, no YAML), explicit write triggers only, and headless review tracks that must read ADRs before asking settled technical-choice questions.

## Requirements

### Requirement: Project ADR directory layout

The system SHALL store project-level architecture decision records under `{DATA_ROOT_DIR}/reports/{project_name}/.notes/`.

That directory MUST contain:

- `index.md` — an index listing only (links and short metadata); it MUST NOT hold full ADR bodies
- zero or more `adr-YYYYMMDD-slug.md` files — one accepted decision body per file

The directory name `.notes` is reserved project report metadata. Workflows and backends that enumerate person folders under `reports/{project_name}/` MUST NOT treat `.notes` as an engineer `display_name` directory.

#### Scenario: Index and ADR files coexist under .notes

- **WHEN** project `game-backend` has one recorded decision dated 2026-07-14 with slug `redis-session`
- **THEN** the files `reports/game-backend/.notes/index.md` and `reports/game-backend/.notes/adr-20260714-redis-session.md` both exist
- **AND** the full decision text lives only in the `adr-*.md` file

#### Scenario: Dot notes is not a person folder

- **WHEN** a workflow lists engineer folders under `reports/game-backend/`
- **THEN** `.notes` MUST NOT be treated as a person report root


<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->

---
### Requirement: ADR index and body contract

Each `adr-YYYYMMDD-slug.md` file MUST start with a Markdown `#` title, then a mandatory `<tl;dr>...</tl;dr>` block, then body sections. The file MUST NOT use YAML frontmatter and MUST NOT use a separate `<meta>` block.

The `<tl;dr>` block MUST use bold-key bullets in the post-bug learning-note style. Required keys:

- `何時要想起這則:`
- `決策:`
- `不要做／不要再問:`
- `要做:`
- `意圖:`

Optional key: `自問（可選）:`.

The TL;DR MUST NOT record `date`, `status`, `source`, or `mr_iid`. Chronology for browsing comes from the filename `adr-YYYYMMDD-…` and the index table date column derived from that filename.

Body after the TL;DR MUST include `## 為何這樣選（意圖）` when the decision came from a manager clarification, then sections covering context, decision, and consequences (Traditional Chinese or English equivalent headings).

Headless readers MUST use the `<tl;dr>` content as the primary signal for whether a technical-choice question is already settled. They MUST NOT require reading later body sections when the TL;DR alone settles the question.

`index.md` MUST be a Markdown table listing each ADR (at least date from the filename, relative filename, one-line summary). Creating a new ADR MUST: (1) write the ADR file, (2) Read any existing `index.md`, (3) append exactly one new row (MUST NOT overwrite the whole index with a blank template). Only when `index.md` is missing MUST the writer create it from the skill’s initial template.

Missing `index.md` on read MUST be treated as an empty decision set. Missing directory on first write MUST be created.

#### Scenario: Empty notes treated as no known decisions

- **WHEN** `reports/game-backend/.notes/index.md` does not exist
- **THEN** reader workflows treat the known-decision set as empty

#### Scenario: Write creates layout with tl;dr

- **WHEN** an agent records the first ADR for a project
- **THEN** it creates `.notes/` if needed, writes `adr-YYYYMMDD-slug.md` containing a `<tl;dr>` block with the required bold keys and no YAML frontmatter, then creates `index.md` with a header row and one data row
- **AND** the TL;DR does not contain `date`, `status`, `source`, or `mr_iid` keys

#### Scenario: Index append does not wipe prior rows

- **GIVEN** `index.md` already has a row for `adr-20260701-postgres.md`
- **WHEN** a second ADR `adr-20260714-redis-session.md` is recorded
- **THEN** `index.md` still contains the postgres row
- **AND** a new row for the redis ADR is appended

##### Example: tl;dr shape (no YAML, no provenance keys)

- **GIVEN** a Redis session decision from MR chat
- **WHEN** the ADR file is written
- **THEN** the file begins with `# …`, then a `<tl;dr>` containing at least `何時要想起這則:`, `決策:`, `不要做／不要再問:`, `要做:`, and `意圖:`
- **AND** the file does not start with `---` YAML fences
- **AND** the TL;DR does not list `date`, `status`, `source`, or `mr_iid`

##### Example: index row shape

- **GIVEN** file `adr-20260714-redis-session.md` titled "Session 用 Redis 而非記憶體"
- **WHEN** the index is updated
- **THEN** `index.md` contains a table row with date `2026-07-14` (from the filename), that filename, and a one-line summary


<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Explicit write trigger only

An agent MUST write or modify files under `.notes/` only when the manager’s message explicitly requests recording an ADR using skill-defined trigger phrases (at least: Traditional Chinese「記成 ADR」「寫入決策」, and English `record as ADR`).

Without such an explicit request, the agent MUST NOT create ADR files or alter `index.md`, even if the conversation contains a technical choice answer.

#### Scenario: Answer without record command does not write

- **WHEN** the manager explains why Redis was chosen but does not use an ADR trigger phrase
- **THEN** no new file is created under `.notes/`

#### Scenario: Explicit record command writes

- **WHEN** the manager says「把這個記成 ADR」after stating the Redis decision
- **THEN** a new `adr-*.md` is written and `index.md` gains a corresponding entry


<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->

---
### Requirement: Headless tracks must read ADRs before asking

Before composing new technical-choice follow-up questions (MR draft suggested questions) or new weekly `## 待確認` bullets about technical selection, the `scan-mrs-headless` and `reviewer-batch` workflows MUST read `{notes_dir}/index.md` when the file exists, and MUST read linked ADR bodies as needed to understand recorded decisions.

Those workflows MUST NOT introduce a new pending question or suggested follow-up whose subject is already settled by an existing project ADR.

Person-level `pending_items` / `_notes.md` remain the store for manager 1:1 open questions; project ADRs MUST NOT be written into `_people/{display_name}/_notes.md`.

#### Scenario: Weekly run skips known technical choice

- **GIVEN** `.notes/index.md` links an ADR deciding Redis for session storage
- **WHEN** the weekly batch would otherwise ask why Redis was chosen
- **THEN** that question MUST NOT appear as a new `## 待確認` bullet

#### Scenario: MR scan skips known decision follow-up

- **GIVEN** the same Redis ADR exists
- **WHEN** an MR review draft is produced
- **THEN** the draft MUST NOT list a suggested follow-up whose sole subject is that Redis choice

<!-- @trace
source: project-adr-notes
updated: 2026-07-14
code:
  - backend/src/runs.rs
  - backend/src/executor.rs
  - skills/project-adr-notes/SKILL.md
  - skills/reviewer-batch/output-contract.md
  - backend/src/mr_reviews.rs
  - skills/reviewer-batch/WORKFLOW.md
  - skills/scan-mrs-headless/output-contract.md
  - skills/scan-mrs-headless/WORKFLOW.md
tests:
  - backend/tests/executor_cancellation.rs
  - backend/tests/identity.rs
-->