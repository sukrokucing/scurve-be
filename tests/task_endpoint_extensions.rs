use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{app, jwt};

mod support;

async fn setup() -> Result<(
    axum::Router,
    sqlx::SqlitePool,
    String,
    Uuid,
    Uuid,
    support::db::TestDb,
)> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Ensure clones always include the latest schema/data migrations.
    sqlx::migrate!().run(&pool).await?;

    std::env::set_var("JWT_SECRET", "task-ext-secret");
    let app = app::create_app(pool.clone()).await?;

    let owner_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Owner', 'owner@example.com', 'hash', 'local', ?, ?)",
    )
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'Project X', '#123456', ?, ?)",
    )
    .bind(project_id.to_string())
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let jwt_config = jwt::JwtConfig {
        secret: std::sync::Arc::new(b"task-ext-secret".to_vec()),
        exp_hours: 1,
    };
    let token = jwt_config.encode(owner_id)?;

    Ok((app, pool, token, owner_id, project_id, test_db))
}

#[tokio::test]
async fn batch_delete_tasks_is_atomic() -> Result<()> {
    let (app, pool, token, _owner_id, project_id, _test_db) = setup().await?;
    let now = Utc::now();

    let task1_id = Uuid::new_v4();
    let task2_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 1', 'todo', ?, ?)",
    )
    .bind(task1_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 2', 'todo', ?, ?)",
    )
    .bind(task2_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{}/tasks/batch", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "ids": [task1_id, task2_id] }).to_string(),
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let parsed: Value = serde_json::from_slice(&body)?;
    assert_eq!(parsed["deleted"], 2);

    let deleted_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE project_id = ? AND deleted_at IS NOT NULL",
    )
    .bind(project_id.to_string())
    .fetch_one(&pool)
    .await?;
    assert_eq!(deleted_count, 2);

    let task3_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 3', 'todo', ?, ?)",
    )
    .bind(task3_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{}/tasks/batch", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "ids": [task3_id, Uuid::new_v4()] }).to_string(),
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let task3_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM tasks WHERE id = ?")
            .bind(task3_id.to_string())
            .fetch_one(&pool)
            .await?;
    assert!(task3_deleted.is_none());

    Ok(())
}

#[tokio::test]
async fn project_assignees_and_task_activity_endpoints_work() -> Result<()> {
    let (app, pool, token, _owner_id, project_id, _test_db) = setup().await?;
    let now = Utc::now();

    let assignee_a = Uuid::new_v4();
    let assignee_b = Uuid::new_v4();
    let task_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Alice', 'alice@example.com', 'hash', 'local', ?, ?)",
    )
    .bind(assignee_a.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Bob', 'bob@example.com', 'hash', 'local', ?, ?)",
    )
    .bind(assignee_b.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, assignee, created_at, updated_at) VALUES (?, ?, 'Task A', 'todo', ?, ?, ?)",
    )
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(assignee_a.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, assignee, created_at, updated_at) VALUES (?, ?, 'Task B', 'todo', ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(assignee_b.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    // Duplicate assignment should still return unique assignees.
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, assignee, created_at, updated_at) VALUES (?, ?, 'Task C', 'todo', ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(assignee_a.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/assignees", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let assignees: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(assignees.len(), 2);

    sqlx::query(
        "INSERT INTO activity_log (id, event_name, description, actor_id, subject_id, occurred_at, properties, severity) VALUES (?, 'task.updated', 'Task updated', ?, ?, ?, ?, 'important')",
    )
    .bind("evt-task-1")
    .bind(assignee_a.to_string())
    .bind(task_id.to_string())
    .bind(now)
    .bind(json!({ "payload": { "old": { "status": "todo" }, "new": { "status": "doing" }}}).to_string())
    .execute(&pool)
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}/activity",
                    project_id, task_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let activity: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(activity.len(), 1);
    assert_eq!(activity[0]["action"], "task.updated");

    Ok(())
}

#[tokio::test]
async fn task_list_filters_and_legacy_progress_route_work() -> Result<()> {
    let (app, pool, token, _owner_id, project_id, _test_db) = setup().await?;
    let now = Utc::now();

    let assignee = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Charlie', 'charlie@example.com', 'hash', 'local', ?, ?)",
    )
    .bind(assignee.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, assignee, start_date, due_date, progress, created_at, updated_at) VALUES (?, ?, 'Backend API', 'todo', ?, '2025-01-02T00:00:00Z', '2025-01-10T00:00:00Z', 10, ?, ?)",
    )
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(assignee.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, start_date, due_date, progress, created_at, updated_at) VALUES (?, ?, 'Backend Worker', 'todo', '2025-01-03T00:00:00Z', '2025-01-12T00:00:00Z', 20, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Frontend UI', 'done', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) VALUES (?, ?, ?, 40, 'legacy route', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?status=todo&q=Backend&sort_by=title&sort_dir=desc&page=1&per_page=1",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let total = response
        .headers()
        .get("x-total-count")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(total, "2");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let tasks: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Backend Worker");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?assignee_id={}&start_from=2025-01-02&start_to=2025-01-02&due_from=2025-01-10&due_to=2025-01-10",
                    project_id, assignee
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let filtered: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["id"], task_id.to_string());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?status=todo,done&sort_by=title&sort_dir=asc",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let total = response
        .headers()
        .get("x-total-count")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(total, "3");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks?sort_dir=down", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/tasks/{}/progress", task_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let progress: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0]["progress"], 40);

    Ok(())
}
