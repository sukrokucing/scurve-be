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
async fn test_batch_update_tasks(pool: SqlitePool) {
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

    // 2. Create Two Tasks
    let task1_id = Uuid::new_v4();
    let task2_id = Uuid::new_v4();

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 1', 'todo', ?, ?)")
        .bind(task1_id)
        .bind(project_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 2', 'todo', ?, ?)")
        .bind(task2_id)
        .bind(project_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

    // 3. Batch Update: Move both tasks to 'doing' and set progress
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/tasks/batch", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "tasks": [
                            {
                                "id": task1_id,
                                "status": "doing",
                                "progress": 50
                            },
                            {
                                "id": task2_id,
                                "status": "doing",
                                "progress": 20
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let tasks: Vec<Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(tasks.len(), 2);
    let t1 = tasks.iter().find(|t| t["id"].as_str().unwrap() == task1_id.to_string()).unwrap();
    let t2 = tasks.iter().find(|t| t["id"].as_str().unwrap() == task2_id.to_string()).unwrap();

    assert_eq!(t1["status"], "doing");
    assert_eq!(t1["progress"], 50);
    assert_eq!(t2["status"], "doing");
    assert_eq!(t2["progress"], 20);

    // 4. Test Transactional Failure (One valid, one invalid)
    // Task 3 is valid, Task 999 is missing
    let task3_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 3', 'todo', ?, ?)")
        .bind(task3_id)
        .bind(project_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/tasks/batch", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "tasks": [
                            {
                                "id": task3_id,
                                "status": "done"
                            },
                            {
                                "id": Uuid::new_v4(), // Non-existent
                                "status": "done"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Verify Task 3 was NOT updated (rollback)
    let task3_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task3_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(task3_status, "todo");
}
