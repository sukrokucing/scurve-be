ALTER TABLE tasks ADD COLUMN completed_at TEXT;

UPDATE tasks
SET completed_at = COALESCE(
    (
        SELECT MIN(tp.created_at)
        FROM task_progress tp
        WHERE tp.task_id = tasks.id
          AND tp.deleted_at IS NULL
          AND tp.progress >= 100
    ),
    CASE
        WHEN lower(trim(tasks.status)) IN ('done', 'completed', 'closed')
             OR tasks.progress >= 100
        THEN tasks.updated_at
        ELSE NULL
    END
)
WHERE tasks.deleted_at IS NULL
  AND tasks.completed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_tasks_project_completed_at
ON tasks(project_id, completed_at)
WHERE deleted_at IS NULL;
