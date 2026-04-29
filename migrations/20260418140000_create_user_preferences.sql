CREATE TABLE IF NOT EXISTS user_preferences (
    user_id TEXT PRIMARY KEY,
    timezone TEXT NOT NULL DEFAULT 'Asia/Jakarta',
    locale TEXT NOT NULL DEFAULT 'id-ID',
    hour_cycle INTEGER NOT NULL DEFAULT 24 CHECK (hour_cycle IN (12, 24)),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
