use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::test]
async fn test_critical_path_basic() -> anyhow::Result<()> {
    let db_path = format!("/apps/scurve-be/tmp/test-db-{}.sqlite", Uuid::new_v4());
    let db_url = format!("sqlite:///{}", db_path);
    let _ = std::fs::File::create(&db_path)?;
    let pool = SqlitePool::connect(&db_url).await?;

    // Setup Schema
    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL, provider TEXT NOT NULL, provider_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL, description TEXT, theme_color TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS tasks (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, due_date TEXT, start_date TEXT, end_date TEXT, duration_days INTEGER, assignee TEXT, parent_id TEXT, progress INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS task_dependencies (
        id TEXT PRIMARY KEY, source_task_id TEXT NOT NULL, target_task_id TEXT NOT NULL, type TEXT NOT NULL DEFAULT 'finish_to_start', created_at TEXT NOT NULL,
        CHECK (source_task_id != target_task_id)
    );").execute(&pool).await?;

    // Setup Data
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'local', datetime('now'), datetime('now'))")
        .bind(user_id).execute(&pool).await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '#000', datetime('now'), datetime('now'))")
        .bind(project_id).bind(user_id).execute(&pool).await?;

    // Tasks with explicit durations (days)
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'A', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(a).bind(project_id).bind(2i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'B', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(b).bind(project_id).bind(3i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'C', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(c).bind(project_id).bind(5i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'D', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(d).bind(project_id).bind(1i64).execute(&pool).await?;

    // Dependencies: A->B, B->C, A->D
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4()).bind(a).bind(b).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4()).bind(b).bind(c).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4()).bind(a).bind(d).execute(&pool).await?;

    // Setup App
    use s_curve::app::AppState;
    use s_curve::routes::projects::get_project_critical_path;
    use s_curve::jwt::{JwtConfig, AuthUser};
    use axum::extract::{State as AxState, Path as AxPath};

    let jwt = JwtConfig { secret: std::sync::Arc::new(b"test-secret".to_vec()), exp_hours: 24 };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(pool.clone(), jwt, event_bus);
    let auth = AuthUser { user_id };

    // Call critical path endpoint
    let path = AxPath(project_id);
    let res = get_project_critical_path(AxState(app_state.clone()), auth.clone(), path).await?;
    let ids = res.0.task_ids;

    // Expect critical path A -> B -> C
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], a);
    assert_eq!(ids[1], b);
    assert_eq!(ids[2], c);

    // Cleanup
    let _ = std::fs::remove_file(db_path);
    Ok(())
}
