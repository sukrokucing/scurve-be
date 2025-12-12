-- RBAC Schema: Roles, Permissions, and Scoped Access Control
-- Integrates with existing activity_log for audit trail

-- =============================================================================
-- CORE TABLES
-- =============================================================================

-- Roles table
CREATE TABLE IF NOT EXISTS roles (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT,
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Permissions table (resource.action naming convention)
CREATE TABLE IF NOT EXISTS permissions (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT,
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- JUNCTION TABLES
-- =============================================================================

-- Role-Permission assignments
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id         TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id   TEXT NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (role_id, permission_id)
);

-- User-Role assignments
CREATE TABLE IF NOT EXISTS user_roles (
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id         TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, role_id)
);

-- User-Permission direct grants (with optional scope JSON)
CREATE TABLE IF NOT EXISTS user_permissions (
    id              TEXT PRIMARY KEY NOT NULL,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission_id   TEXT NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    scope           TEXT DEFAULT '{}',  -- JSON: {"project_id": "..."} for scoped permissions
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (user_id, permission_id, scope)
);

-- =============================================================================
-- INDEXES
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_perm ON role_permissions(permission_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_user ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role ON user_roles(role_id);
CREATE INDEX IF NOT EXISTS idx_user_permissions_user ON user_permissions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_permissions_perm ON user_permissions(permission_id);

-- =============================================================================
-- SEED DATA: Default Roles
-- =============================================================================

INSERT OR IGNORE INTO roles (id, name, description) VALUES
    ('00000000-0000-0000-0000-000000000001', 'super_admin', 'Full system access, bypasses all permission checks'),
    ('00000000-0000-0000-0000-000000000002', 'admin', 'Administrative access to manage users, roles, and settings'),
    ('00000000-0000-0000-0000-000000000003', 'project_manager', 'Can create and manage projects and tasks'),
    ('00000000-0000-0000-0000-000000000004', 'member', 'Standard user with access to assigned projects'),
    ('00000000-0000-0000-0000-000000000005', 'viewer', 'Read-only access to assigned projects');

-- =============================================================================
-- SEED DATA: Default Permissions
-- =============================================================================

INSERT OR IGNORE INTO permissions (id, name, description) VALUES
    -- Project permissions
    ('10000000-0000-0000-0000-000000000001', 'project.create', 'Create new projects'),
    ('10000000-0000-0000-0000-000000000002', 'project.view', 'View project details'),
    ('10000000-0000-0000-0000-000000000003', 'project.update', 'Modify project settings'),
    ('10000000-0000-0000-0000-000000000004', 'project.delete', 'Delete projects'),

    -- Task permissions
    ('10000000-0000-0000-0000-000000000011', 'task.create', 'Create tasks in a project'),
    ('10000000-0000-0000-0000-000000000012', 'task.view', 'View task details'),
    ('10000000-0000-0000-0000-000000000013', 'task.update', 'Update tasks'),
    ('10000000-0000-0000-0000-000000000014', 'task.delete', 'Delete tasks'),

    -- Progress permissions
    ('10000000-0000-0000-0000-000000000021', 'progress.create', 'Log progress on tasks'),
    ('10000000-0000-0000-0000-000000000022', 'progress.view', 'View progress history'),

    -- User management permissions
    ('10000000-0000-0000-0000-000000000031', 'user.view', 'View user profiles'),
    ('10000000-0000-0000-0000-000000000032', 'user.manage', 'Manage user accounts'),

    -- RBAC management permissions
    ('10000000-0000-0000-0000-000000000041', 'role.view', 'View roles'),
    ('10000000-0000-0000-0000-000000000042', 'role.manage', 'Create/edit/delete roles'),
    ('10000000-0000-0000-0000-000000000043', 'permission.view', 'View permissions'),
    ('10000000-0000-0000-0000-000000000044', 'permission.manage', 'Manage permission assignments');

-- =============================================================================
-- SEED DATA: Role-Permission Mappings
-- =============================================================================

-- Admin role gets management permissions
INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000031'),
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000032'),
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000041'),
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000042'),
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000043'),
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000044');

-- Project Manager gets full project/task access
INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000001'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000002'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000003'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000004'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000011'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000012'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000013'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000014'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000021'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000022');

-- Member gets view + modify (no delete)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000002'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000003'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000012'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000013'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000011'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000021'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000022');

-- Viewer gets read-only
INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES
    ('00000000-0000-0000-0000-000000000005', '10000000-0000-0000-0000-000000000002'),
    ('00000000-0000-0000-0000-000000000005', '10000000-0000-0000-0000-000000000012'),
    ('00000000-0000-0000-0000-000000000005', '10000000-0000-0000-0000-000000000022');
