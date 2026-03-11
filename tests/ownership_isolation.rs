#![allow(clippy::uninlined_format_args)]

use std::sync::Arc;

use axum::{
    body::{self, Body},
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{app, jwt};

mod support;

#[tokio::test]
async fn cross_user_access_is_blocked() {
    let test_db = support::db::cloned_clean_db().await.unwrap();
    let pool = test_db.pool.clone();

    std::env::set_var("JWT_SECRET", "ownership-test-secret");
    let app = app::create_app(pool.clone()).await.unwrap();

    let owner_id = Uuid::new_v4();
    let outsider_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let progress_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, ?, ?, ?, 'local', ?, ?)",
    )
    .bind(owner_id.to_string())
    .bind("Owner")
    .bind("owner@example.com")
    .bind("hash")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, ?, ?, ?, 'local', ?, ?)",
    )
    .bind(outsider_id.to_string())
    .bind("Outsider")
    .bind("outsider@example.com")
    .bind("hash")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, ?, '#123456', ?, ?)",
    )
    .bind(project_id.to_string())
    .bind(owner_id.to_string())
    .bind("Owner Project")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, progress, created_at, updated_at) VALUES (?, ?, ?, 'pending', 0, ?, ?)",
    )
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind("Owner Task")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) VALUES (?, ?, ?, 10, 'seed', ?, ?)",
    )
    .bind(progress_id.to_string())
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    // Give outsider broad RBAC route permissions so strict mode cannot mask ownership checks.
    for permission_name in [
        "project.view",
        "project.update",
        "task.view",
        "progress.view",
    ] {
        let permission_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
            .bind(permission_name)
            .fetch_one(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO user_permissions (id, user_id, permission_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(outsider_id.to_string())
        .bind(permission_id)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
    }

    let jwt_config = jwt::JwtConfig {
        secret: Arc::new(b"ownership-test-secret".to_vec()),
        exp_hours: 1,
    };
    let outsider_token = jwt_config.encode(outsider_id).unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}", project_id))
                .header("Authorization", format!("Bearer {}", outsider_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/plan", project_id))
                .header("Authorization", format!("Bearer {}", outsider_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!([{ "date": "2026-01-01T00:00:00Z", "planned_progress": 20 }]).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", outsider_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}/progress",
                    project_id, task_id
                ))
                .header("Authorization", format!("Bearer {}", outsider_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}/progress/{}",
                    project_id, task_id, progress_id
                ))
                .header("Authorization", format!("Bearer {}", outsider_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/progress", project_id))
                .header("Authorization", format!("Bearer {}", outsider_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Sanity check the original data was not modified.
    let plan_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_plan WHERE project_id = ?")
            .bind(project_id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(plan_count, 0);

    let body_bytes = body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["error"], "not_found");
}
