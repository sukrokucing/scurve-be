#![allow(clippy::uninlined_format_args)]

mod support;

use uuid::Uuid;

#[tokio::test]
async fn test_critical_path_basic() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Setup Data
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string()).bind(user_id.to_string()).execute(&pool).await?;

    // Tasks with explicit durations (days)
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'A', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(a.to_string()).bind(project_id.to_string()).bind(2i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'B', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(b.to_string()).bind(project_id.to_string()).bind(3i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'C', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(c.to_string()).bind(project_id.to_string()).bind(5i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'D', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(d.to_string()).bind(project_id.to_string()).bind(1i64).execute(&pool).await?;

    // Dependencies: A->B, B->C, A->D
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(a.to_string()).bind(b.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(b.to_string()).bind(c.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(a.to_string()).bind(d.to_string()).execute(&pool).await?;

    // Setup App
    use axum::extract::{Path as AxPath, State as AxState};
    use s_curve::app::AppState;
    use s_curve::jwt::{AuthUser, JwtConfig};
    use s_curve::routes::projects::get_project_critical_path;

    let jwt = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret".to_vec()),
        exp_hours: 24,
    };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(
        pool.clone(),
        jwt,
        event_bus,
        s_curve::authz::RoutePermissionCache::new(),
    );
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

    Ok(())
}
