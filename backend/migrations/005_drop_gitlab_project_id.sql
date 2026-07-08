ALTER TABLE projects DROP COLUMN gitlab_project_id;

INSERT INTO schema_version (version) VALUES (5);
