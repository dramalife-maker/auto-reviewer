CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE people (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT    NOT NULL,
    avatar_seed  TEXT,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE person_identities (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id   INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    label       TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (kind, value)
);

CREATE INDEX idx_identities_person ON person_identities(person_id);
CREATE INDEX idx_identities_lookup ON person_identities(kind, value);

CREATE TABLE projects (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT    NOT NULL UNIQUE,
    repo_path      TEXT    NOT NULL,
    git_remote_url TEXT,
    default_branch TEXT,
    is_git_repo    INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE participation (
    project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    person_id    INTEGER NOT NULL REFERENCES people(id)   ON DELETE CASCADE,
    last_seen_at TEXT,
    PRIMARY KEY (project_id, person_id)
);

CREATE TABLE schedule_config (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    enabled         INTEGER NOT NULL DEFAULT 1,
    cadence         TEXT    NOT NULL DEFAULT 'weekly',
    weekday         INTEGER,
    run_time        TEXT    NOT NULL DEFAULT '09:00',
    mr_poll_interval_min INTEGER NOT NULL DEFAULT 60,
    per_project_timeout_sec INTEGER NOT NULL DEFAULT 600,
    max_concurrency INTEGER NOT NULL DEFAULT 2,
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO schedule_config (id, weekday) VALUES (1, 0);

CREATE TABLE unmatched_authors (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    project_id  INTEGER REFERENCES projects(id) ON DELETE SET NULL,
    first_seen  TEXT    NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT    NOT NULL DEFAULT (datetime('now')),
    commit_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE (kind, value)
);

CREATE TABLE runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger     TEXT    NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'running',
    started_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,
    duration_sec INTEGER,
    project_total INTEGER,
    project_skipped INTEGER NOT NULL DEFAULT 0,
    note        TEXT
);

CREATE INDEX idx_runs_started ON runs(started_at DESC);

CREATE TABLE reports (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    person_id      INTEGER NOT NULL REFERENCES people(id)   ON DELETE CASCADE,
    run_id         INTEGER REFERENCES runs(id) ON DELETE SET NULL,
    report_date    TEXT    NOT NULL,
    report_md_path TEXT    NOT NULL,
    summary_md_path TEXT   NOT NULL,
    one_line       TEXT,
    mr_count       INTEGER,
    commit_count   INTEGER,
    is_read        INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, person_id, report_date)
);

CREATE INDEX idx_reports_person_date ON reports(person_id, report_date DESC);
CREATE INDEX idx_reports_unread ON reports(is_read) WHERE is_read = 0;

CREATE TABLE pending_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id   INTEGER NOT NULL REFERENCES people(id)    ON DELETE CASCADE,
    project_id  INTEGER NOT NULL REFERENCES projects(id)  ON DELETE CASCADE,
    report_id   INTEGER REFERENCES reports(id) ON DELETE SET NULL,
    question    TEXT    NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'open',
    raised_date TEXT    NOT NULL,
    resolved_date TEXT,
    resolution_note TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_pending_person_status ON pending_items(person_id, status);

CREATE TABLE run_projects (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    state       TEXT    NOT NULL DEFAULT 'queued',
    started_at  TEXT,
    finished_at TEXT,
    duration_sec INTEGER,
    error       TEXT,
    UNIQUE (run_id, project_id)
);

CREATE INDEX idx_run_projects_run ON run_projects(run_id);

CREATE TABLE mr_reviews (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    person_id       INTEGER REFERENCES people(id) ON DELETE SET NULL,
    mr_iid          INTEGER NOT NULL,
    mr_title        TEXT,
    review_round    INTEGER NOT NULL DEFAULT 1,
    draft_md_path   TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'draft',
    published_at    TEXT,
    published_body  TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, mr_iid, review_round)
);

CREATE INDEX idx_mr_reviews_inbox ON mr_reviews(status, created_at DESC)
    WHERE status = 'draft';

INSERT INTO schema_version (version) VALUES (1);
