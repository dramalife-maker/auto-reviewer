-- Cron run_time is interpreted in this timezone (offset from UTC, minutes).
-- Default 480 = Asia/Taipei (UTC+8). Previously cron fired in UTC.
ALTER TABLE schedule_config ADD COLUMN tz_offset_min INTEGER NOT NULL DEFAULT 480;

INSERT INTO schema_version (version) VALUES (3);
