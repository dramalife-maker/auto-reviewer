CREATE TABLE mr_review_chat_messages (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    mr_review_id  INTEGER NOT NULL REFERENCES mr_reviews(id) ON DELETE CASCADE,
    role          TEXT    NOT NULL,
    content       TEXT    NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_mr_review_chat_messages_review
  ON mr_review_chat_messages(mr_review_id);

INSERT INTO schema_version (version) VALUES (13);
