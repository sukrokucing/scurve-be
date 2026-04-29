-- ============================================================
-- menus: application navigation menu definitions
-- ============================================================
-- Single source of truth for all navigation items.
-- RBAC: required_permission = NULL means visible to all authenticated users.
-- surfaces and keywords are JSON arrays stored as TEXT.
--
-- PostgreSQL migration notes:
--   • Change `INTEGER NOT NULL DEFAULT 0` → `BOOLEAN NOT NULL DEFAULT FALSE`
--     for hidden/disabled columns.
--   • Change `TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP` → `TIMESTAMPTZ NOT NULL DEFAULT NOW()`
--     for timestamp columns (update all migrations similarly).
--   • `TEXT` JSON columns → `JSONB` for indexed JSON queries.
--   • `lower(hex(randomblob(16)))` → `gen_random_uuid()::text` for UUID generation.
-- ============================================================

CREATE TABLE menus (
    id                  TEXT PRIMARY KEY,
    label               TEXT NOT NULL,
    description         TEXT,
    route               TEXT NOT NULL,
    section             TEXT NOT NULL CHECK (section IN ('main', 'settings')),
    priority            INTEGER NOT NULL DEFAULT 50,
    surfaces            TEXT NOT NULL DEFAULT '[]',      -- JSON array e.g. ["sidebar","search"]
    icon                TEXT NOT NULL,
    keywords            TEXT NOT NULL DEFAULT '[]',      -- JSON array for search indexing
    required_permission TEXT,                            -- NULL = no gate (visible to all)
    hidden              INTEGER NOT NULL DEFAULT 0,      -- 1 = excluded from all responses
    disabled            INTEGER NOT NULL DEFAULT 0,      -- 1 = visible but non-interactive
    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Single-row version counter used for ETag cache invalidation.
-- Bump `version` whenever menus change (admin endpoint or manual migration).
-- CHECK (id = 1) enforces at most one row.
CREATE TABLE menu_version (
    id          INTEGER PRIMARY KEY DEFAULT 1,
    version     INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (id = 1)
);

INSERT INTO menu_version (id, version, updated_at)
VALUES (1, 1, CURRENT_TIMESTAMP);

-- ============================================================
-- Seed: navigation menu entries
-- ============================================================
INSERT INTO menus (id, label, description, route, section, priority, surfaces, icon, keywords, required_permission, hidden, disabled, created_at, updated_at) VALUES
    ('dashboard',
     'Dashboard',
     'Portfolio health and S-curve snapshots',
     '/', 'main', 10,
     '["sidebar","bottom-nav","search"]',
     'LayoutDashboard',
     '["home","overview","summary"]',
     NULL, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('projects',
     'Projects',
     'Manage project setup, members, and rates',
     '/projects', 'main', 20,
     '["sidebar","bottom-nav","search"]',
     'FolderKanban',
     '["portfolio","delivery","roadmap"]',
     'project.view', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('tasks',
     'Tasks',
     'Quick create and execution tracking',
     '/tasks', 'main', 30,
     '["sidebar","bottom-nav","search"]',
     'ListTodo',
     '["todo","work items","execution"]',
     'task.view', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('settings',
     'Settings',
     'Workspace access and governance controls',
     '/settings', 'settings', 90,
     '["sidebar","bottom-nav","search"]',
     'Settings',
     '["admin","configuration","workspace"]',
     NULL, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('settings-users',
     'Users',
     'Search users and assign access',
     '/settings/users', 'settings', 40,
     '["settings-nav","search"]',
     'Users',
     '["members","accounts","people"]',
     'user.manage', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('settings-roles',
     'Roles',
     'Define role bundles and permissions',
     '/settings/roles', 'settings', 41,
     '["settings-nav","search"]',
     'Shield',
     '["rbac","permissions","access control"]',
     'role.manage', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('settings-policy',
     'Policy',
     'Review and edit permission matrix',
     '/settings/policy', 'settings', 42,
     '["settings-nav","search"]',
     'Lock',
     '["rules","governance","authorization"]',
     'role.manage', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

    ('settings-flow',
     'Access Flow',
     'Inspect inheritance and role links',
     '/settings/flow', 'settings', 43,
     '["settings-nav","search"]',
     'GitMerge',
     '["inheritance","hierarchy","relationship flow"]',
     'user.manage', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- ============================================================
-- RBAC: register menu.view permission
-- ============================================================
INSERT OR IGNORE INTO permissions (id, name, description, created_at, updated_at)
VALUES (lower(hex(randomblob(16))), 'menu.view', 'Access navigation menus', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- Assign menu.view to all existing roles so every authenticated user
-- can reach GET /menus regardless of their role.
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
CROSS JOIN permissions p
WHERE p.name = 'menu.view';

-- ============================================================
-- Route permissions: GET /menus → menu.view
-- ============================================================
INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name, created_at, updated_at)
VALUES ('20260416-0001-4000-8000-000000000001', '/menus', 'GET', 'menu.view', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
