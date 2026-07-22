-- Scope a run_project to a single person for manual single-person reruns.
-- NULL person_id keeps the existing batch semantics (all resolved authors)
-- for manual_all, manual_project, MR, scheduled, and poll runs. Existing rows
-- become NULL automatically; no backfill needed.
ALTER TABLE run_projects ADD COLUMN person_id INTEGER REFERENCES people(id);

INSERT INTO schema_version (version) VALUES (15);
