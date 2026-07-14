## ADDED Requirements

### Requirement: Project .notes directory is reserved metadata

Under `{DATA_ROOT_DIR}/reports/{project_name}/`, the directory named `.notes` is reserved for project-level ADR storage as defined by the `project-adr-notes` capability.

Any logic that treats immediate children of `reports/{project_name}/` as engineer folders keyed by `display_name` MUST skip `.notes`.

This requirement does not change the person-level layout under `reports/_people/`, which remains the home of `index.md`, monthly growth files, and `_notes.md` pending history.

#### Scenario: Trends and report scans ignore .notes as a person

- **WHEN** code or a workflow enumerates person directories beneath `reports/game-backend/`
- **THEN** `.notes` is skipped and is not loaded as person trends or weekly person roots

#### Scenario: People notes remain under _people

- **WHEN** a pending item is resolved for Alice
- **THEN** the historical pending line is still written to `reports/_people/Alice Chen/_notes.md` and NOT to `reports/{project}/.notes/`

