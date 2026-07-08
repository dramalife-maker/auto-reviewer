ALTER TABLE projects ADD COLUMN source_type TEXT NOT NULL DEFAULT 'gitlab';
ALTER TABLE projects ADD COLUMN gitlab_project_id TEXT;
ALTER TABLE projects ADD COLUMN default_branches TEXT;

UPDATE projects
SET default_branches = json_array(default_branch)
WHERE default_branch IS NOT NULL AND default_branches IS NULL;

INSERT INTO schema_version (version) VALUES (4);
