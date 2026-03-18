CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    project_id TEXT,
    event_id TEXT NOT NULL,
    actor_id TEXT,
    actor_name TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    change_type TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    read_at TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    deleted_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (project_id) REFERENCES projects(id),
    FOREIGN KEY (actor_id) REFERENCES users(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_user_event
    ON notifications(user_id, event_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_notifications_user_occurred
    ON notifications(user_id, occurred_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_notifications_user_unread
    ON notifications(user_id, read_at, occurred_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_notifications_project_user
    ON notifications(project_id, user_id)
    WHERE deleted_at IS NULL;
