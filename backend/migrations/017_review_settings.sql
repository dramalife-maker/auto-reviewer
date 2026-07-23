-- Global review file ignore list. Single row (id = 1), mirroring
-- `schedule_config`: UI-editable settings live in their own table so they are
-- never clobbered by the projects.yaml upsert that owns `projects`.
--
-- `ignore_globs` holds raw git pathspec patterns as a JSON string array; the
-- `:(exclude)` magic prefix is applied at call time, never stored. Default is
-- an empty list — an ignore list nobody configured must not silently hide
-- files from review.

CREATE TABLE review_settings (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    ignore_globs TEXT NOT NULL DEFAULT '[]',
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO review_settings (id) VALUES (1);

INSERT INTO schema_version (version) VALUES (17);
