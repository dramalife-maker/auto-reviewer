ALTER TABLE projects ADD COLUMN mr_review_skip_labels TEXT NOT NULL DEFAULT '["wip","do-not-review","no-ai-review"]';
ALTER TABLE projects ADD COLUMN mr_review_require_label TEXT;

INSERT INTO schema_version (version) VALUES (7);
