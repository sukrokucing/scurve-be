-- Add timeline/Gantt-compatible fields to tasks
ALTER TABLE tasks ADD COLUMN start_date TEXT;
ALTER TABLE tasks ADD COLUMN end_date TEXT;
ALTER TABLE tasks ADD COLUMN duration_days INTEGER;
ALTER TABLE tasks ADD COLUMN assignee TEXT;
ALTER TABLE tasks ADD COLUMN progress INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 100);

CREATE INDEX IF NOT EXISTS idx_tasks_start_date ON tasks(start_date);
CREATE INDEX IF NOT EXISTS idx_tasks_project_start ON tasks(project_id, start_date);

-- Keep duration_days in sync when start/end are present
CREATE TRIGGER IF NOT EXISTS trg_tasks_set_duration_insert
AFTER INSERT ON tasks
WHEN NEW.start_date IS NOT NULL AND NEW.end_date IS NOT NULL
BEGIN
  UPDATE tasks
  SET duration_days = CAST(julianday(NEW.end_date) - julianday(NEW.start_date) AS INTEGER)
  WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tasks_set_duration_update
AFTER UPDATE OF start_date, end_date ON tasks
WHEN NEW.start_date IS NOT NULL AND NEW.end_date IS NOT NULL
BEGIN
  UPDATE tasks
  SET duration_days = CAST(julianday(NEW.end_date) - julianday(NEW.start_date) AS INTEGER)
  WHERE id = NEW.id;
END;
