## ADDED Requirements

### Requirement: Run detail includes project outputs summary

For runs whose `status` is not `running`, each project entry on `GET /api/runs/{id}` MUST include an `outputs` object (or omit/`null` when empty) that summarizes artifacts produced for that `(run_id, project_id)`:

- `mr_drafts`: `{ "count": <number> }` when the run layout drafts directory for that project contains one or more `*.md` files; otherwise omit or `null`
- `weekly_reports`: `{ "people": [ { "person_id": <number>, "display_name": <string> }, ... ] }` listing people with a `reports` row for that `run_id` and `project_id`; empty list MUST omit `weekly_reports` or set it to `null`

While the run `status` is `running`, every project entry MUST omit `outputs` or set it to `null`.

Missing or unreadable drafts directories MUST NOT fail the whole response; treat draft count as zero.

Project `state` (including `failed` / `skipped_timeout`) MUST NOT suppress `outputs` when artifacts exist.

#### Scenario: Finished MR run exposes draft count

- **WHEN** a finished run has at least one `*.md` under that project's run drafts directory
- **THEN** that project's `outputs.mr_drafts.count` equals the number of those markdown files

#### Scenario: Finished weekly run exposes people from reports

- **WHEN** a finished run has `reports` rows for the project with that `run_id`
- **THEN** that project's `outputs.weekly_reports.people` includes each person's `person_id` and `display_name`

#### Scenario: Running run omits outputs

- **WHEN** a client calls `GET /api/runs/{id}` for a run with `status` `running`
- **THEN** each project entry omits `outputs` or sets it to `null`

#### Scenario: Missing drafts directory yields no mr_drafts

- **WHEN** a finished MR run project has no drafts directory
- **THEN** `outputs.mr_drafts` is omitted or `null`
- **AND** the response status is 200

##### Example: outputs shape

| drafts `*.md` | reports people | outputs (truncated) |
| --- | --- | --- |
| 2 files | none | `{ "mr_drafts": { "count": 2 }, "weekly_reports": null }` |
| none | Alice (id 1) | `{ "mr_drafts": null, "weekly_reports": { "people": [{ "person_id": 1, "display_name": "Alice" }] } }` |
| none | none | `null` / omitted |

### Requirement: Execution history detail shows outputs navigation hints

The run detail「專案結果」view MUST show, for each project with non-empty `outputs`:

- When `mr_drafts.count` is greater than zero: text stating that N MR drafts were produced, with a link to `/mr-inbox` labeled as the MR inbox
- When `weekly_reports.people` is non-empty: text stating weekly reports were produced for those people, with each of the first eight `display_name` values linking to `/reports/{person_id}`; if more than eight people, the UI MUST indicate the remaining count (for example「…等共 N 人」)

Hints MUST appear regardless of project `state` when `outputs` is present. Projects without `outputs` MUST NOT show an empty outputs section.

#### Scenario: MR draft hint links to inbox

- **WHEN** the manager opens a finished run whose project has `outputs.mr_drafts.count` = 2
- **THEN** the project card shows that 2 MR drafts were produced
- **AND** a link to `/mr-inbox` is available

#### Scenario: Weekly report hint links to people

- **WHEN** the manager opens a finished run whose project lists Alice and Bob under `outputs.weekly_reports.people`
- **THEN** the project card shows their names as links to `/reports/{person_id}`

#### Scenario: No outputs hides hints

- **WHEN** a project entry has no `outputs`
- **THEN** the project card does not render an outputs navigation hint block
