use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::test]
async fn create_update_task_with_timeline() -> anyhow::Result<()> {
    // This integration test expects a running test database and the server
    // helpers. We'll spawn the CLI migrator to prepare a temporary sqlite DB
    // referenced by DATABASE_URL env var for this test.

    let db_path = format!("/apps/scurve-be/tmp/test-db-{}.sqlite", Uuid::new_v4());
    // Use three slashes for absolute sqlite file paths (sqlite:///path)
    let db_url = format!("sqlite:///{}", db_path);

    // Ensure the DB file exists (create empty file) so sqlite can open it
    let _ = std::fs::File::create(&db_path)?;

    // Connect and create minimal schema needed for the test (avoid running full migrations)
    let pool = SqlitePool::connect(&db_url).await?;

    // Create minimal tables required for handlers to operate
    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        email TEXT NOT NULL,
        provider TEXT NOT NULL,
        provider_id TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT
    );")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        name TEXT NOT NULL,
        theme_color TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT
    );")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL,
        title TEXT NOT NULL,
        status TEXT NOT NULL,
        due_date TEXT,
        start_date TEXT,
        end_date TEXT,
        duration_days INTEGER,
        assignee TEXT,
        parent_id TEXT,
        progress INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT
    );")
        .execute(&pool)
        .await?;

    // Add triggers for duration_days (mimicking migration 202511190001)
    sqlx::query("CREATE TRIGGER IF NOT EXISTS trg_tasks_set_duration_insert
        AFTER INSERT ON tasks
        WHEN NEW.start_date IS NOT NULL AND NEW.end_date IS NOT NULL
        BEGIN
          UPDATE tasks
          SET duration_days = CAST(julianday(NEW.end_date) - julianday(NEW.start_date) AS INTEGER)
          WHERE id = NEW.id;
        END;")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE TRIGGER IF NOT EXISTS trg_tasks_set_duration_update
        AFTER UPDATE OF start_date, end_date ON tasks
        WHEN NEW.start_date IS NOT NULL AND NEW.end_date IS NOT NULL
        BEGIN
          UPDATE tasks
          SET duration_days = CAST(julianday(NEW.end_date) - julianday(NEW.start_date) AS INTEGER)
          WHERE id = NEW.id;
        END;")
        .execute(&pool)
        .await?;

    // Create a test user and project directly in DB to avoid depending on auth flows
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'local', datetime('now'), datetime('now'))")
        .bind(user_id)
        .execute(&pool)
        .await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '#000', datetime('now'), datetime('now'))")
        .bind(project_id)
        .bind(user_id)
        .execute(&pool)
        .await?;

    // Build an AppState and call handlers directly (no HTTP server needed)
    use s_curve::app::AppState;
    use s_curve::routes::tasks::{create_task, update_task};
    use s_curve::models::task::{TaskCreateRequest, TaskUpdateRequest};
    use s_curve::jwt::JwtConfig;
    use axum::extract::State as AxState;
    use axum::Json as AxJson;
    use axum::extract::Path as AxPath;

    let jwt = JwtConfig { secret: std::sync::Arc::new(b"test-secret".to_vec()), exp_hours: 24 };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(pool.clone(), jwt, event_bus);

    // Create payload
    let payload = TaskCreateRequest {
        title: "Timeline task".to_string(),
        status: None,
        due_date: None,
        start_date: Some(chrono::DateTime::parse_from_rfc3339("2025-10-01T09:00:00Z")?.with_timezone(&chrono::Utc)),
        end_date: Some(chrono::DateTime::parse_from_rfc3339("2025-10-05T17:00:00Z")?.with_timezone(&chrono::Utc)),
        assignee: None,
        parent_id: None,
        progress: Some(5),
    };

    let path = AxPath(project_id);
    let auth = s_curve::jwt::AuthUser { user_id };

    let (status, json_resp) = create_task(AxState(app_state.clone()), path, auth.clone(), axum::http::HeaderMap::new(), AxJson(payload)).await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let created = json_resp.0;
    assert_eq!(created.title, "Timeline task");
    assert_eq!(created.progress, 5);
    assert!(created.start_date.is_some());

    // Update with invalid date range
    // Update with invalid date range
    let bad_update = TaskUpdateRequest { title: None, status: None, due_date: None, start_date: Some(chrono::DateTime::parse_from_rfc3339("2025-10-10T00:00:00Z")?.with_timezone(&chrono::Utc)), end_date: Some(chrono::DateTime::parse_from_rfc3339("2025-10-05T00:00:00Z")?.with_timezone(&chrono::Utc)), assignee: None, parent_id: None, progress: None };

    let path = AxPath((project_id, created.id));
    let res = update_task(AxState(app_state.clone()), auth.clone(), axum::http::HeaderMap::new(), path, AxJson(bad_update)).await;
    assert!(res.is_err());

    // Update with invalid progress
    let bad_progress = TaskUpdateRequest { title: None, status: None, due_date: None, start_date: None, end_date: None, assignee: None, parent_id: None, progress: Some(150) };
    let path = AxPath((project_id, created.id));
    let res = update_task(AxState(app_state.clone()), auth, axum::http::HeaderMap::new(), path, AxJson(bad_progress)).await;
    assert!(res.is_err());

    // Valid update to check re-fetch and duration_days
    let valid_update = TaskUpdateRequest {
        title: Some("Updated Title".to_string()),
        status: None,
        due_date: None,
        start_date: Some(chrono::DateTime::parse_from_rfc3339("2025-11-01T09:00:00Z")?.with_timezone(&chrono::Utc)),
        end_date: Some(chrono::DateTime::parse_from_rfc3339("2025-11-03T17:00:00Z")?.with_timezone(&chrono::Utc)),
        assignee: None,
        parent_id: None,
        progress: Some(50),
    };
    let auth = s_curve::jwt::AuthUser { user_id };
    let path = AxPath((project_id, created.id));
    let res = update_task(AxState(app_state.clone()), auth.clone(), axum::http::HeaderMap::new(), path, AxJson(valid_update)).await?;
    let updated_task = res.0;
    assert_eq!(updated_task.title, "Updated Title");
    assert_eq!(updated_task.progress, 50);
    // 2025-11-03 17:00 - 2025-11-01 09:00 = ~2.33 days -> 2
    assert_eq!(updated_task.duration_days, Some(2));

    // Verify Sorting
    use s_curve::routes::tasks::{list_tasks, TaskListQuery};

    // Create another task with earlier start date
    let task2_req = TaskCreateRequest {
        title: "Early Task".to_string(),
        status: None,
        due_date: None,
        start_date: Some(chrono::DateTime::parse_from_rfc3339("2025-09-01T09:00:00Z")?.with_timezone(&chrono::Utc)),
        end_date: Some(chrono::DateTime::parse_from_rfc3339("2025-09-05T17:00:00Z")?.with_timezone(&chrono::Utc)),
        assignee: None,
        parent_id: None,
        progress: Some(0),
    };
    let path = AxPath(project_id);
    let (status, _) = create_task(AxState(app_state.clone()), path, auth.clone(), axum::http::HeaderMap::new(), AxJson(task2_req)).await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);

    // List tasks
    let query = TaskListQuery { progress: None, task_id: None };
    let path = AxPath(project_id);
    let res = list_tasks(AxState(app_state.clone()), path, axum::extract::Query(query), auth).await?;
    let tasks = res.0;

    assert_eq!(tasks.len(), 2);
    // Should be sorted by start_date ASC. Early Task (Sept) first, Updated Task (Nov) second.
    assert_eq!(tasks[0].title, "Early Task");
    assert_eq!(tasks[1].title, "Updated Title");


    // Cleanup file
    let _ = std::fs::remove_file(db_path);

    Ok(())
}
