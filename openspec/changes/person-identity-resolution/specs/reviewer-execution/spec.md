## ADDED Requirements

### Requirement: Weekly manifest includes resolved authors

Before spawning the reviewer-batch subprocess, the backend SHALL write `manifest.json` including an `authors` array. Each element MUST contain `email` (normalized git author email), `git_name` (raw `%an`), `person_id` (integer), and `display_name` (canonical `people.display_name`).

Only authors with a resolved `person_id` MUST appear in `authors`. Unresolved authors MUST NOT appear in the array.

#### Scenario: Manifest lists only resolved authors

- **WHEN** a project has commits from `alice@co.com` (bound) and `bob@gmail.com` (unbound) in the analysis window
- **THEN** manifest `authors` contains only the entry for Alice

##### Example: manifest authors shape

- **GIVEN** person id 1 with `display_name` "Alice Chen" bound to `git_email: alice@co.com`
- **WHEN** the weekly manifest is written for that project
- **THEN** `authors` contains `{ "email": "alice@co.com", "git_name": "Alice", "person_id": 1, "display_name": "Alice Chen" }`

---

### Requirement: Reviewer-batch workflow uses manifest authors

The reviewer-batch workflow SHALL determine the set of engineers to report on exclusively from `manifest.authors`. It MUST NOT enumerate git authors independently to decide person groupings.

For each `authors[]` entry, report files MUST be written under `{report_root}/{display_name}/{run_date}/`.

#### Scenario: Workflow skips git log person discovery

- **WHEN** manifest `authors` contains one entry with `display_name` "Alice Chen"
- **THEN** the workflow produces reports only under `Alice Chen/` and does not create directories for other git display names

## MODIFIED Requirements

### Requirement: Summary files are parsed into reports and pending items

After a successful project run, the backend SHALL scan `$DATA_ROOT_DIR/reports/<name>/<person>/<YYYY-MM-DD>/summary.md` files produced by the skill.

For each summary file, the parser MUST read YAML frontmatter fields `person`, `project`, `date`, `one_line`, `mr_count`, `commit_count`, resolve `person_id` by matching `people.display_name` to frontmatter `person`, upsert `reports` for `(project_id, person_id, report_date)`, and insert `pending_items` for each bullet under heading `## 待確認`.

If frontmatter `person` does not match any existing `people.display_name`, the parser MUST skip that summary file and MUST NOT create a new `people` row.

#### Scenario: Parse summary with two pending questions

- **WHEN** a summary file contains frontmatter and two bullets under `## 待確認` and `person` matches an existing person
- **THEN** one `reports` row exists and two `pending_items` rows exist with `status='open'`

##### Example: frontmatter and pending bullets

- **GIVEN** summary frontmatter `person: Alice`, `date: 2026-07-05`, `one_line: Stable week` and a `people` row with `display_name='Alice'`
- **WHEN** the parser processes the file with two `-` lines under `## 待確認`
- **THEN** `reports.one_line` is `Stable week` and `pending_items` count for that person is 2

#### Scenario: Unknown person in summary is skipped

- **WHEN** summary frontmatter `person` is `Ghost` and no `people` row has that display name
- **THEN** no `reports` row is created for that file

