-- Optimizes task list endpoint access paths:
-- /projects/{project_id}/tasks with deleted_at guard, filters, and sorting.

CREATE INDEX IF NOT EXISTS idx_tasks_project_deleted
    ON tasks(project_id, deleted_at);

CREATE INDEX IF NOT EXISTS idx_tasks_project_deleted_status
    ON tasks(project_id, deleted_at, status);

CREATE INDEX IF NOT EXISTS idx_tasks_project_deleted_assignee
    ON tasks(project_id, deleted_at, assignee);

CREATE INDEX IF NOT EXISTS idx_tasks_project_deleted_due_date
    ON tasks(project_id, deleted_at, due_date);

CREATE INDEX IF NOT EXISTS idx_tasks_project_deleted_updated_at
    ON tasks(project_id, deleted_at, updated_at);
