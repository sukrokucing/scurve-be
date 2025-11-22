use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{app, jwt};

#[sqlx::test]
async fn test_project_plan_management(pool: SqlitePool) {
    std::env::set_var("JWT_SECRET", "test_secret");
    let app = app::create_app(pool.clone()).await.unwrap();

    // 1. Setup: Create User and Project
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Test User', 'test@example.com', 'hash', 'local', ?, ?)")
        .bind(user_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'Test Project', '#000000', ?, ?)")
        .bind(project_id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

    let jwt_config = jwt::JwtConfig {
        secret: std::sync::Arc::new(b"test_secret".to_vec()),
        exp_hours: 1,
    };
    let token = jwt_config.encode(user_id).unwrap();

    // 2. Create Plan (Update)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/plan", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!([
                        {
                            "date": "2025-01-01T00:00:00Z",
                            "planned_progress": 10
                        },
                        {
                            "date": "2025-02-01T00:00:00Z",
                            "planned_progress": 50
                        }
                    ])
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let plan: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0]["planned_progress"], 10);
    assert_eq!(plan[1]["planned_progress"], 50);

    // 3. Verify Dashboard
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/dashboard", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let dashboard: Value = serde_json::from_slice(&body).unwrap();
    let dashboard_plan = dashboard["plan"].as_array().unwrap();
    assert_eq!(dashboard_plan.len(), 2);

    // 4. Replace Plan
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/plan", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!([
                        {
                            "date": "2025-03-01T00:00:00Z",
                            "planned_progress": 100
                        }
                    ])
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let plan: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0]["planned_progress"], 100);

    // 5. Clear Plan
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{}/plan", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify Empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/dashboard", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let dashboard: Value = serde_json::from_slice(&body).unwrap();
    let dashboard_plan = dashboard["plan"].as_array().unwrap();
    assert_eq!(dashboard_plan.len(), 0);
}
