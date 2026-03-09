-- Project membership model + task.description guarantee.
-- - Introduces project_members with soft-delete support.
-- - Introduces a project_owner role seeded into RBAC.
-- - Adds tasks.description (non-null) with migration/backfill strategy.

-- ---------------------------------------------------------------------------
-- Project members
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

CREATE INDEX IF NOT EXISTS idx_project_members_project
    ON project_members(project_id);
CREATE INDEX IF NOT EXISTS idx_project_members_user
    ON project_members(user_id);
CREATE INDEX IF NOT EXISTS idx_project_members_role
    ON project_members(role_id);
CREATE INDEX IF NOT EXISTS idx_project_members_active_project
    ON project_members(project_id, user_id, role_id)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_project_members_active_unique
    ON project_members(project_id, user_id)
    WHERE deleted_at IS NULL;

-- Add a dedicated project owner role and map it to existing project/task/progress
-- permissions. This keeps ownership semantics inside RBAC roles.
INSERT OR IGNORE INTO roles (id, name, description)
VALUES (
    '00000000-0000-0000-0000-000000000006',
    'project_owner',
    'Owner role for a specific project scope'
);

INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000002'), -- project.view
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000003'), -- project.update
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000004'), -- project.delete
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000011'), -- task.create
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000012'), -- task.view
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000013'), -- task.update
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000014'), -- task.delete
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000021'), -- progress.create
    ('00000000-0000-0000-0000-000000000006', '10000000-0000-0000-0000-000000000022'); -- progress.view

-- Backfill membership for all existing project owners.
INSERT OR IGNORE INTO project_members (
    id, project_id, user_id, role_id, created_at, created_by, updated_at, updated_by
)
SELECT
    lower(hex(randomblob(16))),
    p.id,
    p.user_id,
    '00000000-0000-0000-0000-000000000006',
    CURRENT_TIMESTAMP,
    NULL,
    CURRENT_TIMESTAMP,
    NULL
FROM projects p
INNER JOIN users u ON u.id = p.user_id
WHERE p.deleted_at IS NULL AND u.deleted_at IS NULL;

-- ---------------------------------------------------------------------------
-- Task description hardening
-- ---------------------------------------------------------------------------
-- Existing legacy rows get a migrated description marker.
ALTER TABLE tasks ADD COLUMN description TEXT NOT NULL DEFAULT '';
UPDATE tasks
SET description = '[Migrated] ' || COALESCE(NULLIF(TRIM(title), ''), 'Untitled Task')
WHERE description IS NULL OR TRIM(description) = '';

-- Auto-fill quick-add description on create when omitted/blank.
DROP TRIGGER IF EXISTS trg_tasks_description_autofill_insert;
CREATE TRIGGER trg_tasks_description_autofill_insert
AFTER INSERT ON tasks
WHEN NEW.description IS NULL OR TRIM(NEW.description) = ''
BEGIN
  UPDATE tasks
  SET description = '[Quick Add] ' || COALESCE(NULLIF(TRIM(NEW.title), ''), 'Untitled Task')
  WHERE id = NEW.id;
END;

-- Prevent empty descriptions on update.
DROP TRIGGER IF EXISTS trg_tasks_description_non_empty_update;
CREATE TRIGGER trg_tasks_description_non_empty_update
BEFORE UPDATE OF description ON tasks
WHEN NEW.description IS NULL OR TRIM(NEW.description) = ''
BEGIN
  SELECT RAISE(ABORT, 'description must not be empty');
END;
