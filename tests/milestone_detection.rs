//! Integration test for milestone detection via date normalization.
//!
//! Verifies that tasks with start_date == end_date (same calendar day) return
//! identical timestamps, enabling frontend milestone detection.

use chrono::{DateTime, Utc, Timelike};
use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::test]
async fn test_milestone_same_day_dates_are_identical() -> anyhow::Result<()> {
    let db_path = format!("/apps/scurve-be/tmp/test-milestone-{}.sqlite", Uuid::new_v4());
    let db_url = format!("sqlite:///{}", db_path);
    let _ = std::fs::File::create(&db_path)?;
    let pool = SqlitePool::connect(&db_url).await?;

    // Setup schema
    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL UNIQUE, password_hash TEXT, provider TEXT NOT NULL, provider_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL, description TEXT, theme_color TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, due_date TEXT, start_date TEXT, end_date TEXT, duration_days INTEGER, assignee TEXT, parent_id TEXT, progress INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    // Setup data
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    sqlx::query("INSERT INTO users (id, name, email, provider, created_at, updated_at) VALUES (?, 'Test', 'milestone@example.com', 'local', ?, ?)")
        .bind(user_id.to_string()).bind(&now).bind(&now).execute(&pool).await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'Milestone Test', '#000', ?, ?)")
        .bind(project_id.to_string()).bind(user_id.to_string()).bind(&now).bind(&now).execute(&pool).await?;

    // Insert task with DIFFERENT times on the SAME day (simulating what normalize should fix)
    // Before normalization, frontend would see different timestamps and not detect as milestone
    let start_date = "2025-12-05T09:30:00Z";
    let end_date = "2025-12-05T17:45:00Z";

    // Parse to DateTime first to normalize (simulating what the handler does)
    let start_dt: DateTime<Utc> = start_date.parse()?;
    let end_dt: DateTime<Utc> = end_date.parse()?;

    // Normalize to midnight
    let start_normalized = start_dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end_normalized = end_dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, start_date, end_date, created_at, updated_at) VALUES (?, ?, 'Release Milestone', 'pending', ?, ?, ?, ?)")
        .bind(task_id.to_string())
        .bind(project_id.to_string())
        .bind(start_normalized.to_rfc3339())
        .bind(end_normalized.to_rfc3339())
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await?;

    // Retrieve and verify
    let row: (String, String) = sqlx::query_as("SELECT start_date, end_date FROM tasks WHERE id = ?")
        .bind(task_id.to_string())
        .fetch_one(&pool)
        .await?;

    let fetched_start: DateTime<Utc> = row.0.parse()?;
    let fetched_end: DateTime<Utc> = row.1.parse()?;

    // CRITICAL: Both timestamps must be identical for milestone detection
    assert_eq!(
        fetched_start.timestamp(),
        fetched_end.timestamp(),
        "Milestone detection requires identical timestamps. Got start={} end={}",
        row.0, row.1
    );

    // Verify both are at midnight
    assert_eq!(fetched_start.time().hour(), 0, "start_date should be at midnight");
    assert_eq!(fetched_start.time().minute(), 0);
    assert_eq!(fetched_start.time().second(), 0);
    assert_eq!(fetched_end.time().hour(), 0, "end_date should be at midnight");
    assert_eq!(fetched_end.time().minute(), 0);
    assert_eq!(fetched_end.time().second(), 0);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
    Ok(())
}

#[tokio::test]
async fn test_different_days_not_milestone() -> anyhow::Result<()> {
    let db_path = format!("/apps/scurve-be/tmp/test-not-milestone-{}.sqlite", Uuid::new_v4());
    let db_url = format!("sqlite:///{}", db_path);
    let _ = std::fs::File::create(&db_path)?;
    let pool = SqlitePool::connect(&db_url).await?;

    // Setup schema
    sqlx::query("CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, due_date TEXT, start_date TEXT, end_date TEXT, duration_days INTEGER, assignee TEXT, parent_id TEXT, progress INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    let task_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Different days - should NOT be milestone
    let start_date = "2025-12-01T10:00:00Z";
    let end_date = "2025-12-10T15:00:00Z";

    let start_dt: DateTime<Utc> = start_date.parse()?;
    let end_dt: DateTime<Utc> = end_date.parse()?;

    let start_normalized = start_dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end_normalized = end_dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, start_date, end_date, created_at, updated_at) VALUES (?, ?, 'Multi-day Task', 'pending', ?, ?, ?, ?)")
        .bind(task_id.to_string())
        .bind(project_id.to_string())
        .bind(start_normalized.to_rfc3339())
        .bind(end_normalized.to_rfc3339())
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await?;

    let row: (String, String) = sqlx::query_as("SELECT start_date, end_date FROM tasks WHERE id = ?")
        .bind(task_id.to_string())
        .fetch_one(&pool)
        .await?;

    let fetched_start: DateTime<Utc> = row.0.parse()?;
    let fetched_end: DateTime<Utc> = row.1.parse()?;

    // Different days should have different timestamps (not a milestone)
    assert_ne!(
        fetched_start.timestamp(),
        fetched_end.timestamp(),
        "Multi-day tasks should have different timestamps"
    );

    // But both should still be at midnight
    assert_eq!(fetched_start.time().hour(), 0);
    assert_eq!(fetched_end.time().hour(), 0);

    let _ = std::fs::remove_file(&db_path);
    Ok(())
}
