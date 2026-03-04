-- Frontend telemetry ingestion storage.
-- Idempotency key is event_id (PRIMARY KEY) from client payload.

CREATE TABLE IF NOT EXISTS telemetry_events (
    event_id                 TEXT PRIMARY KEY NOT NULL,
    event_name               TEXT NOT NULL,
    occurred_at              DATETIME NOT NULL,
    session_id               TEXT,
    route                    TEXT,
    user_id                  TEXT NOT NULL,
    project_id               TEXT,
    view                     TEXT,
    outcome                  TEXT,
    reason                   TEXT,
    duration_ms              INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    intent_to_complete_ms    INTEGER CHECK (intent_to_complete_ms IS NULL OR intent_to_complete_ms >= 0),
    metadata                 JSON,
    ingested_by              TEXT NOT NULL,
    created_at               DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_telemetry_events_name_occurred
    ON telemetry_events(event_name, occurred_at);

CREATE INDEX IF NOT EXISTS idx_telemetry_events_user_occurred
    ON telemetry_events(user_id, occurred_at);

CREATE INDEX IF NOT EXISTS idx_telemetry_events_project_occurred
    ON telemetry_events(project_id, occurred_at);
