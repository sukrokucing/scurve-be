#![allow(clippy::uninlined_format_args)]

mod support;

use uuid::Uuid;

#[tokio::test]
async fn test_task_dependencies() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Setup Data
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let task1_id = Uuid::new_v4();
    let task2_id = Uuid::new_v4();
    let task3_id = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string()).bind(user_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'T1', 'todo', datetime('now'), datetime('now'))")
        .bind(task1_id.to_string()).bind(project_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'T2', 'todo', datetime('now'), datetime('now'))")
        .bind(task2_id.to_string()).bind(project_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'T3', 'todo', datetime('now'), datetime('now'))")
        .bind(task3_id.to_string()).bind(project_id.to_string()).execute(&pool).await?;

    // Setup App
    use axum::extract::{Path as AxPath, State as AxState};
    use axum::Json as AxJson;
    use s_curve::app::AppState;
    use s_curve::jwt::{AuthUser, JwtConfig};
    use s_curve::models::dependency::DependencyCreateRequest;
    use s_curve::routes::tasks::{create_dependency, delete_dependency, list_dependencies};

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

    // 1. Create Dependency T1 -> T2
    let payload = DependencyCreateRequest {
        source_task_id: task1_id,
        target_task_id: task2_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let (status, json) = create_dependency(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        AxJson(payload),
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let dep_id = json.0.id;

    // 2. List Dependencies
    let path = AxPath(project_id);
    let res = list_dependencies(AxState(app_state.clone()), path, auth.clone()).await?;
    let deps = res.0;
    // Debug: check raw table count
    let total: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM task_dependencies")
        .fetch_one(&pool)
        .await?;
    println!("raw task_dependencies count = {}", total);
    let total_project: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM task_dependencies d INNER JOIN tasks t ON t.id = d.source_task_id WHERE t.project_id = ?").bind(project_id.to_string()).fetch_one(&pool).await?;
    println!("project task_dependencies count = {}", total_project);
    // (Removed test diagnostics) The handler uses a CASE-based SELECT to textify UUIDs when needed.
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
    let res = create_dependency(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        AxJson(payload),
    )
    .await;
    assert!(res.is_err()); // Should fail with bad request

    // 3b. Test deeper cycle: create T2 -> T3, then attempt T3 -> T1 when T1->T2 exists
    // First, recreate T1->T2 (already exists). Create T2->T3
    let payload = DependencyCreateRequest {
        source_task_id: task2_id,
        target_task_id: task3_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let (status, _json) = create_dependency(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        AxJson(payload),
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);

    // Now attempt to create T3 -> T1 which would form a cycle T1->T2->T3->T1
    let payload = DependencyCreateRequest {
        source_task_id: task3_id,
        target_task_id: task1_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let res = create_dependency(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        AxJson(payload),
    )
    .await;
    assert!(res.is_err()); // Should fail with deep cycle detection

    // 4. Try Self Dependency T1 -> T1
    let payload = DependencyCreateRequest {
        source_task_id: task1_id,
        target_task_id: task1_id,
        type_: "finish_to_start".to_string(),
    };
    let path = AxPath(project_id);
    let res = create_dependency(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        AxJson(payload),
    )
    .await;
    assert!(res.is_err());

    // 5. Delete Dependency
    let path = AxPath((project_id, dep_id));
    let status = delete_dependency(AxState(app_state.clone()), path, auth.clone()).await?;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    // 6. Verify Deletion
    let path = AxPath(project_id);
    let res = list_dependencies(AxState(app_state.clone()), path, auth.clone()).await?;
    // One dependency (T2->T3) remains after deleting the original T1->T2
    assert_eq!(res.0.len(), 1);
    assert_eq!(res.0[0].source_task_id, task2_id);

    Ok(())
}
