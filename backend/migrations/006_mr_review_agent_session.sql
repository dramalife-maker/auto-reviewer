ALTER TABLE mr_reviews ADD COLUMN agent_session_id TEXT;
ALTER TABLE mr_reviews ADD COLUMN reviewer_agent TEXT NOT NULL DEFAULT 'cursor';

INSERT INTO schema_version (version) VALUES (6);
