-- Add composite index on activity_log(subject_id, event_name, occurred_at DESC)
--
-- Without this index, GET /projects/:id/tasks/:id/activity performs a full table
-- scan on every request. With it, SQLite seeks directly to rows matching the
-- (subject_id, event_name prefix) pair and reads them in reverse-chronological
-- order without a sort step.
--
-- PostgreSQL note: DESC in composite index keys is valid syntax in PostgreSQL too.
CREATE INDEX IF NOT EXISTS idx_activity_log_subject
    ON activity_log(subject_id, event_name, occurred_at DESC);
