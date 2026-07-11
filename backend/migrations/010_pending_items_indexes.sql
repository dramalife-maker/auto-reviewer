CREATE INDEX idx_pending_person_project_status
  ON pending_items(person_id, project_id, status);

CREATE UNIQUE INDEX idx_pending_open_unique
  ON pending_items(person_id, project_id, question)
  WHERE status = 'open';

CREATE TABLE IF NOT EXISTS app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
