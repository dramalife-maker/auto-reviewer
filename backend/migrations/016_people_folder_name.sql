-- Introduce an immutable `folder_name` path key for people.
--
-- `display_name` was doubling as the on-disk directory name, the summary.md
-- frontmatter `person` value, and the ingest resolution key. Renaming a person
-- therefore drifted every path and broke ingest re-resolution. `folder_name` is
-- set once at creation (== initial display_name) and never changes, so all
-- report paths stay valid across renames without any filesystem move.
--
-- Backward compatibility: many INSERT sites (production `create_person` plus
-- numerous test helpers) write only `display_name`. The AFTER INSERT trigger
-- backfills `folder_name = display_name` for any row inserted without an
-- explicit non-empty `folder_name`, so those paths keep working and the
-- NOT NULL invariant holds. The trigger fires on INSERT only, preserving
-- immutability under UPDATE (rename).

ALTER TABLE people ADD COLUMN folder_name TEXT NOT NULL DEFAULT '';

UPDATE people SET folder_name = display_name;

CREATE UNIQUE INDEX idx_people_folder_name ON people(folder_name);

CREATE TRIGGER people_folder_name_backfill
AFTER INSERT ON people
FOR EACH ROW WHEN NEW.folder_name = ''
BEGIN
    UPDATE people SET folder_name = NEW.display_name WHERE id = NEW.id;
END;

INSERT INTO schema_version (version) VALUES (16);
