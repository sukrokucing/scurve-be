-- Create task dependencies table for Gantt charts
CREATE TABLE IF NOT EXISTS task_dependencies (
    id TEXT PRIMARY KEY,
    source_task_id TEXT NOT NULL REFERENCES tasks(id),
    target_task_id TEXT NOT NULL REFERENCES tasks(id),
    type TEXT NOT NULL DEFAULT 'finish_to_start',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (source_task_id != target_task_id)
);

CREATE INDEX IF NOT EXISTS idx_task_deps_source ON task_dependencies(source_task_id);
CREATE INDEX IF NOT EXISTS idx_task_deps_target ON task_dependencies(target_task_id);
