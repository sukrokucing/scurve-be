ALTER TABLE tasks ADD COLUMN completed_at_is_backfilled INTEGER NOT NULL DEFAULT 0;

UPDATE tasks
SET completed_at_is_backfilled = 1
WHERE deleted_at IS NULL
  AND completed_at IS NOT NULL;
