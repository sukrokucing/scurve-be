CREATE TABLE activity_log (
    id           TEXT PRIMARY KEY NOT NULL, -- UUID stored as TEXT
    event_name   TEXT NOT NULL,
    description  TEXT NOT NULL,
    actor_id     TEXT, -- Nullable UUID
    subject_id   TEXT, -- Nullable UUID
    occurred_at  DATETIME NOT NULL,
    properties   JSON NOT NULL,
    severity     TEXT NOT NULL
);

-- Index for querying logs by time (useful for retention policy and recent history)
CREATE INDEX idx_activity_log_occurred_at ON activity_log(occurred_at);

-- Index for finding logs related to a specific user (actor)
CREATE INDEX idx_activity_log_actor_id ON activity_log(actor_id);
