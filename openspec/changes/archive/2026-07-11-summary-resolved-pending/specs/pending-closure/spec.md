## ADDED Requirements

### Requirement: Weekly ingest resolves open pending via shared closure semantics

When weekly summary ingestion resolves an open pending item because its question appears under `## 已釐清`, the system MUST apply the same closure field updates as `PATCH /api/pending-items/{id}` for an open row: set `status` to `resolved`, set `resolved_date` to the schedule-timezone month `YYYY-MM`, and leave `resolution_note` null when the summary does not supply a note.

After a successful database update, the system MUST update `{DATA_ROOT_DIR}/reports/_people/{display_name}/_notes.md` using the same resolved-line rewrite rules as manual closure.

If the notes file write fails after the database update succeeded during ingest, the pending item MUST remain `resolved`, and the ingest MUST continue (notes failure MUST NOT abort the whole project ingest).

#### Scenario: Ingest resolve rewrites matching open notes line

- **GIVEN** `_notes.md` contains `- [2026-07] Why choose A?`
- **AND** a matching open `pending_items` row exists
- **WHEN** weekly ingest resolves that item via `## 已釐清` in month `2026-07`
- **THEN** that notes line becomes `- [2026-07→2026-07] ✓ Why choose A?`
