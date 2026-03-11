-- Hard switch: separate RBAC access role from delivery resource roles,
-- and move economic activity into dedicated work_logs.

PRAGMA foreign_keys = OFF;

-- ---------------------------------------------------------------------------
-- Rebuild project_members: role_id -> access_role_id
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS project_members (
    id          TEXT PRIMARY KEY NOT NULL,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id     TEXT NOT NULL REFERENCES roles(id),
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by  TEXT REFERENCES users(id),
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by  TEXT REFERENCES users(id),
    deleted_at  DATETIME,
    deleted_by  TEXT REFERENCES users(id)
);

DROP TABLE IF EXISTS project_members_new;
CREATE TABLE project_members_new (
    id              TEXT PRIMARY KEY NOT NULL,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    access_role_id  TEXT NOT NULL REFERENCES roles(id),
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by      TEXT REFERENCES users(id),
    updated_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by      TEXT REFERENCES users(id),
    deleted_at      DATETIME,
    deleted_by      TEXT REFERENCES users(id)
);

INSERT INTO project_members_new (
    id, project_id, user_id, access_role_id,
    created_at, created_by, updated_at, updated_by,
    deleted_at, deleted_by
)
-- Compatibility path: copy by ordinal position so both legacy
-- (role_id) and already-migrated (access_role_id) schemas work.
SELECT * FROM project_members;

DROP TABLE project_members;
ALTER TABLE project_members_new RENAME TO project_members;

CREATE INDEX IF NOT EXISTS idx_project_members_project
    ON project_members(project_id);
CREATE INDEX IF NOT EXISTS idx_project_members_user
    ON project_members(user_id);
CREATE INDEX IF NOT EXISTS idx_project_members_access_role
    ON project_members(access_role_id);
CREATE INDEX IF NOT EXISTS idx_project_members_active_project
    ON project_members(project_id, user_id, access_role_id)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_project_members_active_unique
    ON project_members(project_id, user_id)
    WHERE deleted_at IS NULL;

-- ---------------------------------------------------------------------------
-- Resource roles and role rates
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS resource_roles (
    id                  TEXT PRIMARY KEY NOT NULL,
    name                TEXT NOT NULL UNIQUE,
    description         TEXT,
    default_hourly_rate REAL NOT NULL DEFAULT 0 CHECK (default_hourly_rate >= 0),
    currency            TEXT NOT NULL DEFAULT 'USD',
    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at          DATETIME
);

CREATE INDEX IF NOT EXISTS idx_resource_roles_active
    ON resource_roles(deleted_at, name);

CREATE TABLE IF NOT EXISTS project_resource_role_rates (
    id                  TEXT PRIMARY KEY NOT NULL,
    project_id          TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    resource_role_id    TEXT NOT NULL REFERENCES resource_roles(id),
    hourly_rate         REAL NOT NULL CHECK (hourly_rate >= 0),
    currency            TEXT NOT NULL,
    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by          TEXT REFERENCES users(id),
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by          TEXT REFERENCES users(id),
    deleted_at          DATETIME,
    deleted_by          TEXT REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_project_resource_role_rates_project
    ON project_resource_role_rates(project_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_project_resource_role_rates_role
    ON project_resource_role_rates(resource_role_id, deleted_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_project_resource_role_rates_active_unique
    ON project_resource_role_rates(project_id, resource_role_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS project_member_resource_roles (
    id                  TEXT PRIMARY KEY NOT NULL,
    membership_id       TEXT NOT NULL REFERENCES project_members(id) ON DELETE CASCADE,
    resource_role_id    TEXT NOT NULL REFERENCES resource_roles(id),
    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by          TEXT REFERENCES users(id),
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by          TEXT REFERENCES users(id),
    deleted_at          DATETIME,
    deleted_by          TEXT REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_project_member_resource_roles_member
    ON project_member_resource_roles(membership_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_project_member_resource_roles_role
    ON project_member_resource_roles(resource_role_id, deleted_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_project_member_resource_roles_active_unique
    ON project_member_resource_roles(membership_id, resource_role_id)
    WHERE deleted_at IS NULL;

-- ---------------------------------------------------------------------------
-- Work logs (economic events)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS work_logs (
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
    source                  TEXT NOT NULL CHECK (source IN ('manual', 'migrated_task_progress')),
    created_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by              TEXT REFERENCES users(id),
    updated_at              DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by              TEXT REFERENCES users(id),
    deleted_at              DATETIME,
    deleted_by              TEXT REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_work_logs_project
    ON work_logs(project_id, deleted_at, work_date);
CREATE INDEX IF NOT EXISTS idx_work_logs_task
    ON work_logs(task_id, deleted_at, work_date);
CREATE INDEX IF NOT EXISTS idx_work_logs_user
    ON work_logs(user_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_work_logs_role
    ON work_logs(resource_role_id, deleted_at);

-- ---------------------------------------------------------------------------
-- Seed default resource roles
-- ---------------------------------------------------------------------------
INSERT OR IGNORE INTO resource_roles (id, name, description, default_hourly_rate, currency)
VALUES
    ('40000000-0000-0000-0000-000000000001', 'unclassified', 'Fallback role for migrated or uncategorized work', 0, 'USD'),
    ('40000000-0000-0000-0000-000000000002', 'system_analyst', 'System analyst project contribution role', 55, 'USD'),
    ('40000000-0000-0000-0000-000000000003', 'backend_engineer', 'Backend engineering project contribution role', 70, 'USD'),
    ('40000000-0000-0000-0000-000000000004', 'frontend_engineer', 'Frontend engineering project contribution role', 65, 'USD'),
    ('40000000-0000-0000-0000-000000000005', 'qa_engineer', 'Quality assurance project contribution role', 50, 'USD'),
    ('40000000-0000-0000-0000-000000000006', 'project_manager', 'Project management contribution role', 80, 'USD');

-- Backfill every active member with unclassified resource capability.
INSERT OR IGNORE INTO project_member_resource_roles (
    id, membership_id, resource_role_id, created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))),
    pm.id,
    '40000000-0000-0000-0000-000000000001',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM project_members pm
WHERE pm.deleted_at IS NULL;

-- Backfill legacy task_progress economics into work_logs.
INSERT INTO work_logs (
    id,
    project_id,
    task_id,
    user_id,
    resource_role_id,
    hours,
    hourly_rate_snapshot,
    currency_snapshot,
    cost_amount,
    work_date,
    note,
    source,
    created_at,
    updated_at
)
SELECT
    lower(hex(randomblob(16))),
    tp.project_id,
    tp.task_id,
    CASE
        WHEN t.assignee IS NOT NULL
             AND EXISTS (SELECT 1 FROM users u WHERE u.id = t.assignee AND u.deleted_at IS NULL)
        THEN t.assignee
        ELSE NULL
    END,
    '40000000-0000-0000-0000-000000000001',
    COALESCE(tp.actual_hours, 0.0),
    CASE
        WHEN tp.actual_hours IS NOT NULL AND tp.actual_hours > 0 AND tp.actual_cost IS NOT NULL
            THEN MAX(tp.actual_cost / tp.actual_hours, 0.0)
        ELSE 0.0
    END,
    'USD',
    COALESCE(tp.actual_cost, 0.0),
    DATE(COALESCE(tp.created_at, CURRENT_TIMESTAMP)),
    tp.note,
    'migrated_task_progress',
    COALESCE(tp.created_at, CURRENT_TIMESTAMP),
    COALESCE(tp.updated_at, CURRENT_TIMESTAMP)
FROM task_progress tp
LEFT JOIN tasks t ON t.id = tp.task_id
WHERE tp.deleted_at IS NULL
  AND (tp.actual_hours IS NOT NULL OR tp.actual_cost IS NOT NULL);

PRAGMA foreign_keys = ON;
