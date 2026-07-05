-- Track provisioning health so a failed clone/fetch isolates one project
-- without aborting startup. `health` is 'healthy' | 'unhealthy'.
ALTER TABLE projects ADD COLUMN health TEXT NOT NULL DEFAULT 'healthy';
ALTER TABLE projects ADD COLUMN health_reason TEXT;

INSERT INTO schema_version (version) VALUES (2);
