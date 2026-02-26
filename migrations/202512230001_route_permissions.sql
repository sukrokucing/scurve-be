-- Create route_permissions table
CREATE TABLE IF NOT EXISTS route_permissions (
    id TEXT PRIMARY KEY NOT NULL,
    route_pattern TEXT NOT NULL,
    method TEXT NOT NULL,
    permission_name TEXT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(route_pattern, method)
);

-- Seed data
INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name, created_at) VALUES
-- Projects
(lower(hex(randomblob(16))), '/projects', 'GET', 'project.view', datetime('now')),
(lower(hex(randomblob(16))), '/projects', 'POST', 'project.create', datetime('now')),
(lower(hex(randomblob(16))), '/projects/:id', 'GET', 'project.view', datetime('now')),
(lower(hex(randomblob(16))), '/projects/:id', 'PUT', 'project.update', datetime('now')),
(lower(hex(randomblob(16))), '/projects/:id', 'DELETE', 'project.delete', datetime('now')),

-- Tasks
(lower(hex(randomblob(16))), '/projects/:project_id/tasks', 'GET', 'task.view', datetime('now')),
(lower(hex(randomblob(16))), '/projects/:project_id/tasks', 'POST', 'task.create', datetime('now')),
(lower(hex(randomblob(16))), '/tasks/:id', 'GET', 'task.view', datetime('now')),
(lower(hex(randomblob(16))), '/tasks/:id', 'PUT', 'task.update', datetime('now')),
(lower(hex(randomblob(16))), '/tasks/:id', 'DELETE', 'task.delete', datetime('now')),

-- Task Dependencies
(lower(hex(randomblob(16))), '/tasks/:id/dependencies', 'POST', 'task.update', datetime('now')),
(lower(hex(randomblob(16))), '/tasks/:id/dependencies/:target_id', 'DELETE', 'task.update', datetime('now')),

-- Progress
(lower(hex(randomblob(16))), '/projects/:project_id/progress', 'GET', 'progress.view', datetime('now')),
(lower(hex(randomblob(16))), '/tasks/:taskId/progress', 'POST', 'progress.create', datetime('now')),

-- Users
(lower(hex(randomblob(16))), '/users', 'GET', 'user.view', datetime('now')),
(lower(hex(randomblob(16))), '/users', 'POST', 'user.manage', datetime('now')),
(lower(hex(randomblob(16))), '/users/:id', 'PUT', 'user.manage', datetime('now')),
(lower(hex(randomblob(16))), '/users/:id', 'DELETE', 'user.manage', datetime('now')),

-- RBAC Roles
(lower(hex(randomblob(16))), '/rbac/roles', 'GET', 'role.view', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/roles', 'POST', 'role.manage', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id', 'GET', 'role.view', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id', 'DELETE', 'role.manage', datetime('now')),

-- RBAC Permissions (Role Assignment)
(lower(hex(randomblob(16))), '/rbac/roles/:role_id/permissions', 'GET', 'role.view', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id/permissions', 'POST', 'role.manage', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/roles/:role_id/permissions/:permission_id', 'DELETE', 'role.manage', datetime('now')),

-- RBAC Permissions (Listing/Creating)
(lower(hex(randomblob(16))), '/rbac/permissions', 'GET', 'permission.view', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/permissions', 'POST', 'permission.manage', datetime('now')),

-- RBAC User Roles
(lower(hex(randomblob(16))), '/rbac/users/:user_id/roles', 'GET', 'role.view', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/roles', 'POST', 'role.manage', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/roles/:role_id', 'DELETE', 'role.manage', datetime('now')),

-- RBAC User Permissions (Direct)
(lower(hex(randomblob(16))), '/rbac/users/:user_id/permissions', 'GET', 'permission.view', datetime('now')),
(lower(hex(randomblob(16))), '/rbac/users/:user_id/permissions', 'POST', 'permission.manage', datetime('now')),

-- RBAC Effective Permissions
(lower(hex(randomblob(16))), '/rbac/users/:user_id/effective-permissions', 'GET', 'user.view', datetime('now')),

-- Audit Logs
(lower(hex(randomblob(16))), '/rbac/audit-logs', 'GET', 'user.view', datetime('now'));
