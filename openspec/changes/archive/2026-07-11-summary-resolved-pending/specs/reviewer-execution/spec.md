## ADDED Requirements

### Requirement: Weekly summary includes resolved section for closed pending

The reviewer-batch `summary.md` contract MUST include a fourth level-2 heading `## 已釐清` after `## 待確認`.

Bullets under `## 已釐清` MUST use `- ` list markers. The section MUST remain valid when empty (heading present, zero bullets).

When the workflow determines that an `open_pending` entry for the current author is resolved this run, it MUST write that entry's `question` text verbatim as a bullet under `## 已釐清`, and MUST NOT also list that question under `## 待確認`.

Omitting a question from both `## 待確認` and `## 已釐清` MUST leave the corresponding open database row unchanged.

#### Scenario: Resolved open issue listed under 已釐清

- **GIVEN** `open_pending` contains `{ "display_name": "Alice Chen", "question": "Why choose A?" }`
- **AND** the workflow judges the issue resolved this week
- **WHEN** the workflow writes Alice Chen's `summary.md`
- **THEN** `## 已釐清` includes a bullet whose text is exactly `Why choose A?`
- **AND** `## 待確認` does not include that text

#### Scenario: Omitted open issue stays open at workflow layer

- **GIVEN** an open pending question for Alice Chen
- **AND** the workflow omits it from both `## 待確認` and `## 已釐清`
- **WHEN** the summary is written
- **THEN** the workflow does not write SQLite
- **AND** closure of that row is not requested by the summary file

### Requirement: Summary ingestion auto-resolves matching open pending from 已釐清

After upserting a weekly `summary.md`, the backend MUST parse bullets under `## 已釐清`.

For each bullet text Q, if an open `pending_items` row exists for the summary's resolved `person_id`, the summary's `project_id`, and `question` exactly equal to Q, the backend MUST resolve that row using the same database fields as manual closure: `status='resolved'`, `resolved_date` set to the schedule-timezone calendar month `YYYY-MM`, and `resolution_note` left null when not provided by the summary.

The backend MUST attempt to sync the person `_notes.md` resolved-line format after a successful database resolve. If notes sync fails, the backend MUST leave the row `resolved`, MUST log a warning, and MUST continue ingesting remaining summaries.

A `## 已釐清` bullet with no matching open row MUST be ignored without failing the ingest of that summary.

#### Scenario: Exact open question in 已釐清 becomes resolved

- **GIVEN** an open pending item for person Alice, project G, question `Why choose A?`
- **WHEN** a weekly summary for Alice and project G is ingested with that exact text under `## 已釐清`
- **THEN** that pending item has `status` equal to `resolved`
- **AND** `resolved_date` matches `YYYY-MM` for the schedule timezone

#### Scenario: 待確認 omission without 已釐清 does not resolve

- **GIVEN** an open pending item for person Alice, project G, question `Why choose A?`
- **WHEN** a weekly summary for Alice and project G is ingested with empty `## 已釐清` and without that question under `## 待確認`
- **THEN** that pending item remains `status` equal to `open`

#### Scenario: Unknown 已釐清 bullet is ignored

- **GIVEN** no open pending item with question `Never seen?`
- **WHEN** a summary is ingested containing `- Never seen?` under `## 已釐清`
- **THEN** ingest succeeds
- **AND** no pending row is created solely from the 已釐清 section
