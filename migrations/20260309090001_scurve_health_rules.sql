-- S-curve health rule engine (configurable stage classification).

CREATE TABLE IF NOT EXISTS s_curve_stage_rule_sets (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    metric      TEXT NOT NULL CHECK (metric IN ('progress', 'hours', 'cost')),
    scope       TEXT NOT NULL CHECK (scope IN ('global', 'project')),
    project_id  TEXT REFERENCES projects(id) ON DELETE CASCADE,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by  TEXT REFERENCES users(id),
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by  TEXT REFERENCES users(id),
    deleted_at  DATETIME,
    deleted_by  TEXT REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS s_curve_stage_rules (
    id                          TEXT PRIMARY KEY NOT NULL,
    rule_set_id                 TEXT NOT NULL REFERENCES s_curve_stage_rule_sets(id) ON DELETE CASCADE,
    priority                    INTEGER NOT NULL,
    stage                       TEXT NOT NULL CHECK (stage IN ('lag', 'log', 'maturity', 'decline')),
    elapsed_from                REAL,
    elapsed_to                  REAL,
    planned_from                REAL,
    planned_to                  REAL,
    actual_from                 REAL,
    actual_to                   REAL,
    variance_from               REAL,
    variance_to                 REAL,
    require_rule_50_70_pass     INTEGER CHECK (require_rule_50_70_pass IN (0, 1)),
    created_at                  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at                  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at                  DATETIME
);

CREATE INDEX IF NOT EXISTS idx_s_curve_rule_sets_metric_scope
    ON s_curve_stage_rule_sets(metric, scope, project_id);
CREATE INDEX IF NOT EXISTS idx_s_curve_rule_sets_active
    ON s_curve_stage_rule_sets(is_active, deleted_at);
CREATE INDEX IF NOT EXISTS idx_s_curve_rules_rule_set_priority
    ON s_curve_stage_rules(rule_set_id, priority);
CREATE UNIQUE INDEX IF NOT EXISTS idx_s_curve_rules_unique_priority
    ON s_curve_stage_rules(rule_set_id, priority)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_s_curve_rule_sets_unique_active
    ON s_curve_stage_rule_sets(metric, COALESCE(project_id, 'GLOBAL'))
    WHERE is_active = 1 AND deleted_at IS NULL;

-- Seed one default global rule set for progress.
INSERT OR IGNORE INTO s_curve_stage_rule_sets (id, name, metric, scope, is_active)
VALUES (
    '30000000-0000-0000-0000-000000000001',
    'Default Progress Stage Rules',
    'progress',
    'global',
    1
);

-- Stage boundaries are intentionally simple and can be replaced by inserting
-- a project-specific or updated global active rule set.
INSERT OR IGNORE INTO s_curve_stage_rules (
    id, rule_set_id, priority, stage,
    variance_from, variance_to,
    created_at, updated_at
) VALUES
    ('31000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000001', 10, 'lag',      NULL, -10.0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('31000000-0000-0000-0000-000000000002', '30000000-0000-0000-0000-000000000001', 20, 'log',     -10.0,   0.0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('31000000-0000-0000-0000-000000000003', '30000000-0000-0000-0000-000000000001', 30, 'maturity',  0.0,  10.0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('31000000-0000-0000-0000-000000000004', '30000000-0000-0000-0000-000000000001', 40, 'decline',  10.0,  NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
