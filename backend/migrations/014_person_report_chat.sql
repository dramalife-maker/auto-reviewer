CREATE TABLE person_report_chats (
    person_id         INTEGER PRIMARY KEY REFERENCES people(id) ON DELETE CASCADE,
    agent_session_id  TEXT,
    reviewer_agent    TEXT    NOT NULL DEFAULT 'cursor',
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE person_report_chat_messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id  INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    role       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_person_report_chat_messages_person
  ON person_report_chat_messages(person_id);

INSERT INTO schema_version (version) VALUES (14);
