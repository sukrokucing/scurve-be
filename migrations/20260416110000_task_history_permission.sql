-- ============================================================
-- task.history: admin-only permission for the task history endpoint
-- ============================================================
-- GET /admin/tasks/:task_id/history returns the full automatic
-- audit trail (create, update, delete) for any task with actor
-- names resolved from the users table.
--
-- Intentionally NOT assigned to viewer / member roles —
-- only admin and super_admin can access task history.
-- ============================================================

INSERT OR IGNORE INTO permissions (id, name, description, created_at, updated_at)
VALUES (
    lower(hex(randomblob(16))),
    'task.history',
    'View full task change history with actor names (admin only)',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
);

-- Assign only to privileged roles
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
CROSS JOIN permissions p
WHERE p.name = 'task.history'
  AND r.name IN ('admin', 'super_admin');

-- Route permission: GET /admin/tasks/:task_id/history
INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name, created_at, updated_at)
VALUES (
    '20260416-1100-4000-8000-000000000001',
    '/admin/tasks/:task_id/history',
    'GET',
    'task.history',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
);
