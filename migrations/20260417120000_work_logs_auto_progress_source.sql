-- Add 'auto_progress' as a valid work_log source.
-- SQLite does not support ALTER COLUMN to change CHECK constraints,
-- so the table is recreated with the updated constraint.

PRAGMA foreign_keys = OFF;

CREATE TABLE work_logs_new (
    id                      TEXT PRIMARY KEY NOT NULL,
    project_id              TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id                 TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    user_id                 TEXT REFERENCES users(id),
    resource_role_id        TEXT NOT NULL REFERENCES resource_roles(id),
    hours                   REAL NOT NULL CHECK (hours >= 0),
    hourly_rate_snapshot    REAL NOT NULL CHECK (hourly_rate_snapshot >= 0),
    currency_snapshot       TEXT NOT NULL,
    cost_amount             REAL NOT NULL CHECK (cost_amount >= 0),
    work_date               TEXT NOT NULL,
    note                    TEXT,
    source                  TEXT NOT NULL CHECK (source IN ('manual', 'migrated_task_progress', 'auto_progress')),
    created_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by              TEXT REFERENCES users(id),
    updated_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by              TEXT REFERENCES users(id),
    deleted_at              DATETIME,
    deleted_by              TEXT REFERENCES users(id)
);

INSERT INTO work_logs_new SELECT * FROM work_logs;

DROP TABLE work_logs;
ALTER TABLE work_logs_new RENAME TO work_logs;

CREATE INDEX IF NOT EXISTS idx_work_logs_project
    ON work_logs(project_id, deleted_at, work_date);
CREATE INDEX IF NOT EXISTS idx_work_logs_task
    ON work_logs(task_id, deleted_at, work_date);
CREATE INDEX IF NOT EXISTS idx_work_logs_user
    ON work_logs(user_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_work_logs_role
    ON work_logs(resource_role_id, deleted_at);

PRAGMA foreign_keys = ON;
