-- Add parent_id to tasks for subtask hierarchy
ALTER TABLE tasks ADD COLUMN parent_id TEXT REFERENCES tasks(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_tasks_parent_id ON tasks(parent_id);
