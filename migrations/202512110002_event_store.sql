-- Phase 6: Append-Only Event Store with Tamper-Evident Hash Chain
-- This table stores immutable domain events for:
-- - Rebuilding projections
-- - Compliance audits
-- - Deterministic history

CREATE TABLE IF NOT EXISTS event_store (
    id              TEXT PRIMARY KEY NOT NULL,
    event_name      TEXT NOT NULL,
    occurred_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    actor_id        TEXT,
    subject_id      TEXT,
    payload         TEXT NOT NULL,  -- JSON
    severity        TEXT NOT NULL DEFAULT 'important',
    -- Tamper-evident: hash = SHA256(prev_hash || payload)
    prev_hash       TEXT,  -- NULL for first event
    hash            TEXT NOT NULL,
    -- Metadata
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Index for querying by event name and severity
CREATE INDEX IF NOT EXISTS idx_event_store_name ON event_store(event_name);
CREATE INDEX IF NOT EXISTS idx_event_store_severity ON event_store(severity);
CREATE INDEX IF NOT EXISTS idx_event_store_occurred ON event_store(occurred_at);
CREATE INDEX IF NOT EXISTS idx_event_store_actor ON event_store(actor_id);
CREATE INDEX IF NOT EXISTS idx_event_store_subject ON event_store(subject_id);

-- Phase 5: Retention Policy - View for stale noise events (older than 7 days)
-- Usage: DELETE FROM activity_log WHERE id IN (SELECT id FROM stale_noise_logs);
CREATE VIEW IF NOT EXISTS stale_noise_logs AS
SELECT id FROM activity_log
WHERE severity = 'noise'
  AND occurred_at < datetime('now', '-7 days');

-- View for stale important events (older than 90 days)
CREATE VIEW IF NOT EXISTS stale_important_logs AS
SELECT id FROM activity_log
WHERE severity = 'important'
  AND occurred_at < datetime('now', '-90 days');
