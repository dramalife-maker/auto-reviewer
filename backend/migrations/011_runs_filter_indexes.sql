-- Support filtered run history lists: WHERE trigger/status + ORDER BY started_at DESC.
CREATE INDEX IF NOT EXISTS idx_runs_status_started ON runs(status, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_runs_trigger_started ON runs(trigger, started_at DESC);
