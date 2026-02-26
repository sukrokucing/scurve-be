-- Normalize route_permissions to the current router/OpenAPI surface.
-- This migration fixes stale patterns from older seeds and guarantees
-- strict mode has complete coverage for all protected endpoints.

-- Rebuild table to ensure canonical schema (including updated_at).
DROP TABLE IF EXISTS route_permissions_new;
CREATE TABLE route_permissions_new (
    id TEXT PRIMARY KEY NOT NULL,
    route_pattern TEXT NOT NULL,
    method TEXT NOT NULL,
    permission_name TEXT NOT NULL REFERENCES permissions(name),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(route_pattern, method)
);

INSERT INTO route_permissions_new (id, route_pattern, method, permission_name) VALUES
-- Projects
(lower(hex(randomblob(16))), '/projects', 'GET', 'project.view'),
(lower(hex(randomblob(16))), '/projects', 'POST', 'project.create'),
(lower(hex(randomblob(16))), '/projects/:id', 'GET', 'project.view'),
(lower(hex(randomblob(16))), '/projects/:id', 'PUT', 'project.update'),
(lower(hex(randomblob(16))), '/projects/:id', 'DELETE', 'project.delete'),
(lower(hex(randomblob(16))), '/projects/:id/dashboard', 'GET', 'project.view'),
(lower(hex(randomblob(16))), '/projects/:id/critical-path', 'GET', 'project.view'),
(lower(hex(randomblob(16))), '/projects/:id/plan', 'POST', 'project.update'),
(lower(hex(randomblob(16))), '/projects/:id/plan', 'DELETE', 'project.update'),

-- Tasks
(lower(hex(randomblob(16))), '/projects/:project_id/tasks', 'GET', 'task.view'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks', 'POST', 'task.create'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:id', 'GET', 'task.view'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:id', 'PUT', 'task.update'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:id', 'DELETE', 'task.delete'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/batch', 'PUT', 'task.update'),

-- Dependencies
(lower(hex(randomblob(16))), '/projects/:project_id/dependencies', 'GET', 'task.view'),
(lower(hex(randomblob(16))), '/projects/:project_id/dependencies', 'POST', 'task.update'),
(lower(hex(randomblob(16))), '/projects/:project_id/dependencies/:id', 'DELETE', 'task.update'),

-- Progress
(lower(hex(randomblob(16))), '/projects/:project_id/progress', 'GET', 'progress.view'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/progress', 'GET', 'progress.view'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/progress', 'POST', 'progress.create'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/progress/:id', 'GET', 'progress.view'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/progress/:id', 'PUT', 'progress.create'),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/progress/:id', 'DELETE', 'progress.create'),

-- Users
(lower(hex(randomblob(16))), '/users', 'GET', 'user.view'),
(lower(hex(randomblob(16))), '/users', 'POST', 'user.manage'),
(lower(hex(randomblob(16))), '/users/:id', 'PUT', 'user.manage'),
(lower(hex(randomblob(16))), '/users/:id', 'DELETE', 'user.manage'),

-- RBAC: Roles
(lower(hex(randomblob(16))), '/rbac/roles', 'GET', 'role.view'),
(lower(hex(randomblob(16))), '/rbac/roles', 'POST', 'role.manage'),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id', 'GET', 'role.view'),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id', 'DELETE', 'role.manage'),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id/permissions', 'GET', 'role.view'),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id/permissions', 'POST', 'role.manage'),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id/permissions/:permission_id', 'DELETE', 'role.manage'),

-- RBAC: Permissions
(lower(hex(randomblob(16))), '/rbac/permissions', 'GET', 'permission.view'),
(lower(hex(randomblob(16))), '/rbac/permissions', 'POST', 'permission.manage'),

-- RBAC: User role/permission management
(lower(hex(randomblob(16))), '/rbac/users/:user_id/roles', 'GET', 'user.view'),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/roles', 'POST', 'user.manage'),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/roles/:role_id', 'DELETE', 'user.manage'),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/permissions', 'GET', 'user.view'),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/permissions', 'POST', 'user.manage'),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/effective-permissions', 'GET', 'user.view'),

-- RBAC: Audit
(lower(hex(randomblob(16))), '/rbac/audit-logs', 'GET', 'user.view');

DROP TABLE IF EXISTS route_permissions;
ALTER TABLE route_permissions_new RENAME TO route_permissions;

CREATE INDEX IF NOT EXISTS idx_route_permissions_method_pattern
    ON route_permissions(method, route_pattern);
CREATE INDEX IF NOT EXISTS idx_route_permissions_permission_name
    ON route_permissions(permission_name);
