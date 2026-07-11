-- Speed up weekly manifest load of open pending by project.
CREATE INDEX IF NOT EXISTS idx_pending_open_by_project
  ON pending_items(project_id, person_id, id)
  WHERE status = 'open';
