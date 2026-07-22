## MODIFIED Requirements

### Requirement: Weekly summary ingestion deduplicates open pending questions

When ingesting `## 待確認` bullets from a weekly `summary.md` into `pending_items`, the backend MUST skip inserting a row when an existing row already has the same `person_id`, `project_id`, and `question` with `status='open'`.

The backend MUST also skip inserting a row when an existing row with the same `person_id`, `project_id`, and `question` — in any status — originates from a report whose report date is the same as or later than the report date of the summary being ingested. This prevents a re-read of an already-processed summary from creating a duplicate row.

A previously `resolved` row with the same question MUST NOT block insertion of a new `open` row when the summary being ingested has a report date later than that of the resolved row's originating report.

When an existing row's originating report cannot be determined because its report reference is `NULL`, that row MUST NOT block insertion, and the backend MUST log a warning naming the person, project, and question.

#### Scenario: Duplicate open question is not inserted again

- **GIVEN** an open pending item exists for person P, project G, question Q
- **WHEN** a weekly summary for the same person and project is ingested containing bullet Q
- **THEN** no additional `pending_items` row is created for Q

#### Scenario: Resolved question may be raised again by a later summary

- **GIVEN** a resolved pending item exists for person P, project G, question Q, originating from a report dated D1
- **WHEN** a weekly summary for the same person and project with report date D2 later than D1 is ingested containing bullet Q
- **THEN** a new open `pending_items` row is created for Q and the resolved row remains

#### Scenario: Re-reading an already-processed summary creates no row

- **GIVEN** a pending item exists for person P, project G, question Q, originating from a report dated D1
- **WHEN** a summary for the same person and project with report date D1 is ingested again containing bullet Q
- **THEN** no additional `pending_items` row is created for Q

#### Scenario: Re-reading an older summary creates no row

- **GIVEN** a resolved pending item exists for person P, project G, question Q, originating from a report dated D2
- **WHEN** a summary for the same person and project with an earlier report date D1 is ingested containing bullet Q
- **THEN** no additional `pending_items` row is created for Q and Q does not return to `open`

#### Scenario: Missing originating report does not block insertion

- **GIVEN** a pending item exists for person P, project G, question Q whose originating report reference is `NULL`
- **WHEN** a weekly summary for the same person and project is ingested containing bullet Q
- **THEN** insertion proceeds and a warning naming person P, project G, and question Q is logged

##### Example: insertion decision by report date

| existing row status | existing row report date | incoming summary report date | new row inserted |
| ------------------- | ------------------------ | ---------------------------- | ---------------- |
| open | 2026-07-05 | 2026-07-12 | no — open row blocks |
| resolved | 2026-07-05 | 2026-07-12 | yes — incoming is later |
| resolved | 2026-07-12 | 2026-07-12 | no — same date is a re-read |
| resolved | 2026-07-12 | 2026-07-05 | no — incoming is older |
| resolved | (null reference) | 2026-07-05 | yes — cannot compare, warn |
