use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::test]
async fn test_task_dependencies() -> anyhow::Result<()> {
    let db_path = format!("/apps/scurve-be/tmp/test-db-{}.sqlite", Uuid::new_v4());
    let db_url = format!("sqlite:///{}", db_path);
    let _ = std::fs::File::create(&db_path)?;
    let pool = SqlitePool::connect(&db_url).await?;

    // Setup Schema
    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL, provider TEXT NOT NULL, provider_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
    );").execute(&pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL, theme_color TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
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
    let task1_id = Uuid::new_v4();
    let task2_id = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'local', datetime('now'), datetime('now'))")
        .bind(user_id).execute(&pool).await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '#000', datetime('now'), datetime('now'))")
        .bind(project_id).bind(user_id).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'T1', 'todo', datetime('now'), datetime('now'))")
        .bind(task1_id).bind(project_id).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'T2', 'todo', datetime('now'), datetime('now'))")
        .bind(task2_id).bind(project_id).execute(&pool).await?;

    // Setup App
    use s_curve::app::AppState;
    use s_curve::routes::tasks::{create_dependency, list_dependencies, delete_dependency};
    use s_curve::models::dependency::DependencyCreateRequest;
    use s_curve::jwt::{JwtConfig, AuthUser};
    use axum::extract::{State as AxState, Path as AxPath};
    use axum::Json as AxJson;

    let jwt = JwtConfig { secret: std::sync::Arc::new(b"test-secret".to_vec()), exp_hours: 24 };
    let app_state = AppState::new(pool.clone(), jwt);
    let auth = AuthUser { user_id };

    // 1. Create Dependency T1 -> T2
    let payload = DependencyCreateRequest {
        source_task_id: task1_id,
        target_task_id: task2_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let (status, json) = create_dependency(AxState(app_state.clone()), path, auth.clone(), AxJson(payload)).await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let dep_id = json.0.id;

    // 2. List Dependencies
    let path = AxPath(project_id);
    let res = list_dependencies(AxState(app_state.clone()), path, auth.clone()).await?;
    let deps = res.0;
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].id, dep_id);
    assert_eq!(deps[0].source_task_id, task1_id);
    assert_eq!(deps[0].target_task_id, task2_id);

    // 3. Try Reverse Dependency (Cycle) T2 -> T1
    let payload = DependencyCreateRequest {
        source_task_id: task2_id,
        target_task_id: task1_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let res = create_dependency(AxState(app_state.clone()), path, auth.clone(), AxJson(payload)).await;
    assert!(res.is_err()); // Should fail with bad request

    // 4. Try Self Dependency T1 -> T1
    let payload = DependencyCreateRequest {
        source_task_id: task1_id,
        target_task_id: task1_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let res = create_dependency(AxState(app_state.clone()), path, auth.clone(), AxJson(payload)).await;
    assert!(res.is_err());

    // 5. Delete Dependency
    let path = AxPath((project_id, dep_id));
    let status = delete_dependency(AxState(app_state.clone()), path, auth.clone()).await?;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    // 6. Verify Deletion
    let path = AxPath(project_id);
    let res = list_dependencies(AxState(app_state.clone()), path, auth.clone()).await?;
    assert_eq!(res.0.len(), 0);

    // Cleanup
    let _ = std::fs::remove_file(db_path);
    Ok(())
}
