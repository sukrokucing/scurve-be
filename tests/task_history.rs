//! Integration tests for GET /admin/tasks/:task_id/history
//!
//! The endpoint returns the automatic audit trail for a task (create/update/delete),
//! with actor names resolved from the users table.
//! Requires `task.history` permission — assigned to admin/super_admin only.
//!
//! NOTE: `cloned_clean_db()` truncates `role_permissions` and `user_roles`.
//! Tests must explicitly grant permissions and seed activity_log entries.

#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use s_curve::create_app;
use s_curve::jwt::JwtConfig;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

const JWT_SECRET: &str = "test-secret-task-history";

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

async fn setup() -> Result<(axum::Router, SqlitePool, support::db::TestDb)> {
    std::env::set_var("JWT_SECRET", JWT_SECRET);
    std::env::set_var("AUTHZ_MODE", "strict");

    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();
    let app = create_app(pool.clone()).await?;

    Ok((app, pool, test_db))
}

fn make_token(user_id: Uuid) -> String {
    let jwt = JwtConfig {
        secret: Arc::new(JWT_SECRET.as_bytes().to_vec()),
        exp_hours: 1,
    };
    jwt.encode(user_id).unwrap()
}

async fn insert_user(pool: &SqlitePool, name: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at)
         VALUES (?, ?, ?, 'hash', 'local', ?, ?)",
    )
    .bind(user_id.to_string())
    .bind(name)
    .bind(format!(
        "{}-{}@test.com",
        name.replace(' ', "-").to_lowercase(),
        &user_id.to_string()[..8]
    ))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    user_id
}

async fn insert_project(pool: &SqlitePool, owner_id: Uuid) -> Uuid {
    let project_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO projects (id, user_id, name, created_at, updated_at)
         VALUES (?, ?, 'History Test Project', ?, ?)",
    )
    .bind(project_id.to_string())
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

async fn insert_task(pool: &SqlitePool, project_id: Uuid, title: &str) -> Uuid {
    let task_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, progress, created_at, updated_at)
         VALUES (?, ?, ?, 'todo', 0, ?, ?)",
    )
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(title)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

/// Write an activity_log entry for a task event.
async fn log_task_event(
    pool: &SqlitePool,
    task_id: Uuid,
    actor_id: Option<Uuid>,
    event_name: &str,
    properties: serde_json::Value,
) {
    let now = chrono::Utc::now();
    let actor_str = actor_id.map(|id| id.to_string());
    sqlx::query(
        "INSERT INTO activity_log (id, event_name, description, actor_id, subject_id, occurred_at, properties, severity)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'info')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(event_name)
    .bind(format!("{} event", event_name))
    .bind(actor_str)
    .bind(task_id.to_string())
    .bind(now)
    .bind(properties.to_string())
    .execute(pool)
    .await
    .unwrap();
}

/// Grant permissions directly to a user.
async fn grant_user_permissions(pool: &SqlitePool, user_id: Uuid, perm_names: &[&str]) {
    let now = chrono::Utc::now();
    for perm_name in perm_names {
        let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
            .bind(perm_name)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| panic!("permission '{}' not found", perm_name));

        sqlx::query(
            "INSERT INTO user_permissions (id, user_id, permission_id, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id.to_string())
        .bind(perm_id)
        .bind(now)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn get_history(
    app: &axum::Router,
    task_id: Uuid,
    token: &str,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(format!("/admin/tasks/{}/history", task_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Unauthenticated request is rejected.
#[tokio::test]
async fn task_history_requires_authentication() -> Result<()> {
    let (app, pool, _db) = setup().await?;
    let owner = insert_user(&pool, "Owner").await;
    let project = insert_project(&pool, owner).await;
    let task = insert_task(&pool, project, "Test Task").await;

    let req = Request::builder()
        .method("GET")
        .uri(format!("/admin/tasks/{}/history", task))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

/// Non-admin user (no task.history) is denied.
#[tokio::test]
async fn task_history_denied_without_task_history_permission() -> Result<()> {
    let (app, pool, _db) = setup().await?;
    let owner = insert_user(&pool, "Owner").await;
    let project = insert_project(&pool, owner).await;
    let task = insert_task(&pool, project, "Test Task").await;

    let viewer = insert_user(&pool, "Viewer").await;
    // Give viewer some permissions but NOT task.history
    grant_user_permissions(&pool, viewer, &["project.view", "task.view"]).await;
    let token = make_token(viewer);

    let (status, _) = get_history(&app, task, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "non-admin must be denied");
    Ok(())
}

/// Admin with task.history sees the full event list.
#[tokio::test]
async fn task_history_returns_events_for_admin() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let actor = insert_user(&pool, "Jimmy").await;
    let project = insert_project(&pool, actor).await;
    let task = insert_task(&pool, project, "Re-design login page").await;

    // Seed three activity_log entries for this task.
    log_task_event(
        &pool,
        task,
        Some(actor),
        "task.created",
        serde_json::json!({ "title": "Re-design login page" }),
    )
    .await;
    log_task_event(
        &pool,
        task,
        Some(actor),
        "task.updated",
        serde_json::json!({ "status": ["todo", "in_progress"] }),
    )
    .await;
    log_task_event(
        &pool,
        task,
        Some(actor),
        "task.updated",
        serde_json::json!({ "title": ["Re-design login page", "Redesign login page v2"] }),
    )
    .await;

    // Admin user with task.history permission.
    let admin = insert_user(&pool, "Admin").await;
    grant_user_permissions(&pool, admin, &["task.history"]).await;
    let token = make_token(admin);

    let (status, body) = get_history(&app, task, &token).await;
    assert_eq!(status, StatusCode::OK);

    let entries = body.as_array().expect("response must be an array");
    assert_eq!(entries.len(), 3, "should return all 3 logged events");

    // Events are newest-first.
    let events: Vec<&str> = entries.iter().map(|e| e["event"].as_str().unwrap()).collect();
    assert!(
        events.contains(&"task.created"),
        "must include task.created"
    );
    assert!(
        events.iter().filter(|&&e| e == "task.updated").count() == 2,
        "must include both task.updated events"
    );

    Ok(())
}

/// Actor name is resolved from the users table.
#[tokio::test]
async fn task_history_resolves_actor_name() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let actor = insert_user(&pool, "Jimmy Doe").await;
    let project = insert_project(&pool, actor).await;
    let task = insert_task(&pool, project, "Actor name test").await;

    log_task_event(
        &pool,
        task,
        Some(actor),
        "task.created",
        serde_json::json!({ "title": "Actor name test" }),
    )
    .await;

    let admin = insert_user(&pool, "Admin").await;
    grant_user_permissions(&pool, admin, &["task.history"]).await;
    let token = make_token(admin);

    let (status, body) = get_history(&app, task, &token).await;
    assert_eq!(status, StatusCode::OK);

    let entry = &body.as_array().unwrap()[0];
    assert_eq!(
        entry["actor_name"].as_str(),
        Some("Jimmy Doe"),
        "actor name must be resolved from users table"
    );
    assert!(
        entry["actor_id"].is_string(),
        "actor_id UUID must be present"
    );

    Ok(())
}

/// Events from other tasks are not included.
#[tokio::test]
async fn task_history_scoped_to_task() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let actor = insert_user(&pool, "Actor").await;
    let project = insert_project(&pool, actor).await;
    let task_a = insert_task(&pool, project, "Task A").await;
    let task_b = insert_task(&pool, project, "Task B").await;

    log_task_event(
        &pool,
        task_a,
        Some(actor),
        "task.created",
        serde_json::json!({ "title": "Task A" }),
    )
    .await;
    log_task_event(
        &pool,
        task_b,
        Some(actor),
        "task.created",
        serde_json::json!({ "title": "Task B" }),
    )
    .await;

    let admin = insert_user(&pool, "Admin").await;
    grant_user_permissions(&pool, admin, &["task.history"]).await;
    let token = make_token(admin);

    let (status, body) = get_history(&app, task_a, &token).await;
    assert_eq!(status, StatusCode::OK);

    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 1, "only task_a's event must be returned");
    assert_eq!(entries[0]["event"].as_str(), Some("task.created"));

    Ok(())
}

/// 404 when the task does not exist (or is soft-deleted).
#[tokio::test]
async fn task_history_returns_404_for_missing_task() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let admin = insert_user(&pool, "Admin").await;
    grant_user_permissions(&pool, admin, &["task.history"]).await;
    let token = make_token(admin);

    let nonexistent = Uuid::new_v4();
    let (status, _) = get_history(&app, nonexistent, &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

/// Empty array when task exists but has no activity entries yet.
#[tokio::test]
async fn task_history_empty_for_task_with_no_events() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let owner = insert_user(&pool, "Owner").await;
    let project = insert_project(&pool, owner).await;
    let task = insert_task(&pool, project, "Silent Task").await;
    // No activity_log entries inserted.

    let admin = insert_user(&pool, "Admin").await;
    grant_user_permissions(&pool, admin, &["task.history"]).await;
    let token = make_token(admin);

    let (status, body) = get_history(&app, task, &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.as_array().unwrap().len(),
        0,
        "no events yet — must return empty array"
    );
    Ok(())
}
