ALTER TABLE tasks ADD COLUMN progress_method TEXT NOT NULL DEFAULT 'manual_percent_legacy';
ALTER TABLE tasks ADD COLUMN blocked_flag INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN blocked_reason TEXT;
ALTER TABLE tasks ADD COLUMN baseline_start_at TEXT;
ALTER TABLE tasks ADD COLUMN baseline_end_at TEXT;
ALTER TABLE tasks ADD COLUMN task_weight REAL NOT NULL DEFAULT 1.0;

UPDATE tasks
SET baseline_start_at = start_date,
    baseline_end_at = CASE
        WHEN start_date IS NOT NULL
         AND COALESCE(end_date, due_date) IS NOT NULL
         AND julianday(COALESCE(end_date, due_date)) > julianday(start_date)
        THEN COALESCE(end_date, due_date)
        ELSE NULL
    END
WHERE deleted_at IS NULL
  AND baseline_start_at IS NULL
  AND baseline_end_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_tasks_project_progress_method
ON tasks(project_id, progress_method)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_tasks_project_baseline
ON tasks(project_id, baseline_start_at, baseline_end_at)
WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS task_progress_components (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    name TEXT NOT NULL,
    component_type TEXT NOT NULL,
    weight REAL NOT NULL CHECK (weight > 0),
    completion_pct REAL NOT NULL CHECK (completion_pct >= 0 AND completion_pct <= 100),
    planned_at TEXT,
    completed_at TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_progress_components_task_active
ON task_progress_components(task_id, sort_order, created_at)
WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS task_health_rule_sets (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK (scope IN ('global', 'project')),
    project_id TEXT REFERENCES projects(id),
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT,
    CHECK ((scope = 'global' AND project_id IS NULL) OR (scope = 'project' AND project_id IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_health_rule_sets_active_global
ON task_health_rule_sets(scope)
WHERE scope = 'global' AND is_active = 1 AND deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_health_rule_sets_active_project
ON task_health_rule_sets(project_id)
WHERE scope = 'project' AND is_active = 1 AND deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS task_health_rules (
    id TEXT PRIMARY KEY,
    rule_set_id TEXT NOT NULL REFERENCES task_health_rule_sets(id),
    health_status TEXT NOT NULL CHECK (health_status IN ('ahead', 'on_track', 'at_risk', 'critical')),
    variance_from REAL,
    variance_to REAL,
    priority INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_health_rules_rule_set_priority
ON task_health_rules(rule_set_id, priority)
WHERE deleted_at IS NULL;

INSERT OR IGNORE INTO task_health_rule_sets (
    id, scope, project_id, is_active, created_at, updated_at
) VALUES (
    '70000000-0000-0000-0000-000000000001',
    'global',
    NULL,
    1,
    datetime('now'),
    datetime('now')
);

INSERT OR IGNORE INTO task_health_rules (
    id, rule_set_id, health_status, variance_from, variance_to, priority, created_at, updated_at
) VALUES
    ('70000000-0000-0000-0000-000000000011', '70000000-0000-0000-0000-000000000001', 'critical', NULL, -25, 1, datetime('now'), datetime('now')),
    ('70000000-0000-0000-0000-000000000012', '70000000-0000-0000-0000-000000000001', 'at_risk', -25, -10, 2, datetime('now'), datetime('now')),
    ('70000000-0000-0000-0000-000000000013', '70000000-0000-0000-0000-000000000001', 'on_track', -10, 10, 3, datetime('now'), datetime('now')),
    ('70000000-0000-0000-0000-000000000014', '70000000-0000-0000-0000-000000000001', 'ahead', 10, NULL, 4, datetime('now'), datetime('now'));
