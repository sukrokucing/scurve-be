#![allow(clippy::uninlined_format_args)]

mod support;

use uuid::Uuid;

#[tokio::test]
async fn create_update_task_with_timeline() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Create a test user and project directly in DB to avoid depending on auth flows
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string())
        .execute(&pool)
        .await?;

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .execute(&pool)
        .await?;

    // Build an AppState and call handlers directly (no HTTP server needed)
    use axum::extract::Path as AxPath;
    use axum::extract::State as AxState;
    use axum::Json as AxJson;
    use s_curve::app::AppState;
    use s_curve::jwt::JwtConfig;
    use s_curve::models::task::{TaskCreateRequest, TaskUpdateRequest};
    use s_curve::routes::tasks::{create_task, update_task};

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

    // Create payload
    let payload = TaskCreateRequest {
        title: "Timeline task".to_string(),
        description: None,
        status: None,
        due_date: None,
        start_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-10-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        end_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-10-05T17:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        assignee: None,
        parent_id: None,
        progress: Some(5),
    };

    let path = AxPath(project_id);
    let auth = s_curve::jwt::AuthUser { user_id };

    let (status, json_resp) = create_task(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        axum::http::HeaderMap::new(),
        AxJson(payload),
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let created = json_resp.0;
    assert_eq!(created.title, "Timeline task");
    assert_eq!(created.progress, 5);
    assert!(created.start_date.is_some());

    // Update with invalid date range
    // Update with invalid date range
    let bad_update = TaskUpdateRequest {
        title: None,
        description: None,
        status: None,
        due_date: None,
        start_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-10-10T00:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        end_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-10-05T00:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        assignee: None,
        parent_id: None,
        progress: None,
    };

    let path = AxPath((project_id, created.id));
    let res = update_task(
        AxState(app_state.clone()),
        auth.clone(),
        axum::http::HeaderMap::new(),
        path,
        AxJson(bad_update),
    )
    .await;
    assert!(res.is_err());

    // Update with invalid progress
    let bad_progress = TaskUpdateRequest {
        title: None,
        description: None,
        status: None,
        due_date: None,
        start_date: None,
        end_date: None,
        assignee: None,
        parent_id: None,
        progress: Some(150),
    };
    let path = AxPath((project_id, created.id));
    let res = update_task(
        AxState(app_state.clone()),
        auth,
        axum::http::HeaderMap::new(),
        path,
        AxJson(bad_progress),
    )
    .await;
    assert!(res.is_err());

    // Valid update to check re-fetch and duration_days
    let valid_update = TaskUpdateRequest {
        title: Some("Updated Title".to_string()),
        description: None,
        status: None,
        due_date: None,
        start_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-11-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        end_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-11-03T17:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        assignee: None,
        parent_id: None,
        progress: Some(50),
    };
    let auth = s_curve::jwt::AuthUser { user_id };
    let path = AxPath((project_id, created.id));
    let res = update_task(
        AxState(app_state.clone()),
        auth.clone(),
        axum::http::HeaderMap::new(),
        path,
        AxJson(valid_update),
    )
    .await?;
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
        description: None,
        status: None,
        due_date: None,
        start_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-09-01T09:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        end_date: Some(
            chrono::DateTime::parse_from_rfc3339("2025-09-05T17:00:00Z")?
                .with_timezone(&chrono::Utc),
        ),
        assignee: None,
        parent_id: None,
        progress: Some(0),
    };
    let path = AxPath(project_id);
    let (status, _) = create_task(
        AxState(app_state.clone()),
        path,
        auth.clone(),
        axum::http::HeaderMap::new(),
        AxJson(task2_req),
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::CREATED);

    // List tasks
    let query = TaskListQuery {
        progress: None,
        task_id: None,
        q: None,
        status: None,
        assignee_id: None,
        start_from: None,
        start_to: None,
        due_from: None,
        due_to: None,
        sort_by: None,
        sort_dir: None,
        page: None,
        per_page: None,
    };
    let path = AxPath(project_id);
    let res = list_tasks(
        AxState(app_state.clone()),
        path,
        axum::extract::Query(query),
        auth,
    )
    .await?;
    let tasks = res.1 .0;

    assert_eq!(tasks.len(), 2);
    // Should be sorted by start_date ASC. Early Task (Sept) first, Updated Task (Nov) second.
    assert_eq!(tasks[0].title, "Early Task");
    assert_eq!(tasks[1].title, "Updated Title");

    Ok(())
}
