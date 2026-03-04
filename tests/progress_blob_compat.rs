use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{create_app, jwt::JwtConfig};

mod support;

#[tokio::test]
async fn progress_endpoints_support_legacy_blob_uuid_rows() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    std::env::set_var("JWT_SECRET", "blob-progress-secret");
    let app = create_app(pool.clone()).await?;

    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let progress_id = Uuid::new_v4();
    let now = Utc::now();
    let now_epoch = now.timestamp();

    // Insert legacy blob UUID rows.
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Blob User', 'blob@example.com', 'hash', 'local', ?, ?)",
    )
    .bind(user_id.as_bytes().to_vec())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'Blob Project', '#111111', ?, ?)",
    )
    .bind(project_id.as_bytes().to_vec())
    .bind(user_id.as_bytes().to_vec())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Blob Task', 'todo', ?, ?)",
    )
    .bind(task_id.as_bytes().to_vec())
    .bind(project_id.as_bytes().to_vec())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) VALUES (?, ?, ?, 25, 'blob note', ?, ?)",
    )
    .bind(progress_id.as_bytes().to_vec())
    .bind(task_id.as_bytes().to_vec())
    .bind(project_id.as_bytes().to_vec())
    // Legacy instances can store unix epoch style timestamps.
    .bind(now_epoch)
    .bind(now_epoch)
    .execute(&pool)
    .await?;

    let jwt = JwtConfig {
        secret: std::sync::Arc::new(b"blob-progress-secret".to_vec()),
        exp_hours: 1,
    };
    let token = jwt.encode(user_id)?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}/progress",
                    project_id, task_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let items: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["task_id"], task_id.to_string());

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
    let items: Vec<Value> = serde_json::from_slice(&body)?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], progress_id.to_string());

    Ok(())
}
