#![allow(clippy::uninlined_format_args)]

use anyhow::{Context, Result};
use axum::{
    body::{self, Body},
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::create_app;

mod support;

#[tokio::test]
async fn task_completion_timestamp_and_schedule_status_are_backend_computed() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    std::env::set_var("JWT_SECRET", "task-completion-secret");
    let app = create_app(pool.clone()).await?;

    let register_body = json!({
        "name": "Completion User",
        "email": "completion-user@example.com",
        "password": "password123"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(register_body.to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let auth: Value = serde_json::from_slice(&body)?;
    let token = auth["token"].as_str().context("missing token")?.to_string();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "name": "Completion Project" }).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let project: Value = serde_json::from_slice(&body)?;
    let project_id = project["id"]
        .as_str()
        .context("missing project id")?
        .to_string();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Ship API",
                        "status": "todo",
                        "due_date": "2099-01-10T00:00:00Z",
                        "progress": 10
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let task: Value = serde_json::from_slice(&body)?;
    let task_id = task["id"].as_str().context("missing task id")?.to_string();
    assert!(task["completed_at"].is_null());
    assert_eq!(task["completed_at_is_backfilled"], false);
    assert_eq!(task["schedule_status"], "on_time");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "done" }).to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let task: Value = serde_json::from_slice(&body)?;
    assert!(task["completed_at"].is_string());
    assert_eq!(task["completed_at_is_backfilled"], false);
    assert_eq!(task["schedule_status"], "finished_early");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Historical Catch-up",
                        "status": "todo",
                        "due_date": "2000-01-01T00:00:00Z",
                        "progress": 0
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let overdue_task: Value = serde_json::from_slice(&body)?;
    let overdue_task_id = overdue_task["id"]
        .as_str()
        .context("missing overdue task id")?
        .to_string();
    assert_eq!(overdue_task["schedule_status"], "overdue");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "No Due Date",
                        "status": "todo"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let no_due_task: Value = serde_json::from_slice(&body)?;
    let no_due_task_id = no_due_task["id"]
        .as_str()
        .context("missing no due task id")?
        .to_string();
    assert_eq!(no_due_task["schedule_status"], "not_specified");

    let legacy_task_id = Uuid::new_v4();
    let legacy_completed_at = "2025-01-01T10:00:00Z";
    let legacy_due_date = "2099-03-01T00:00:00Z";
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO tasks (
            id, project_id, title, description, status, due_date, start_date, end_date,
            assignee, parent_id, progress, completed_at, completed_at_is_backfilled,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(legacy_task_id.to_string())
    .bind(project_id.clone())
    .bind("Legacy Backfilled Task")
    .bind("[Migrated] Legacy Backfilled Task")
    .bind("done")
    .bind(legacy_due_date)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(100i32)
    .bind(legacy_completed_at)
    .bind(1i64)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/projects/{}/tasks/{}/progress",
                    project_id, overdue_task_id
                ))
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "progress": 100,
                        "note": "completed from progress feed"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}",
                    project_id, overdue_task_id
                ))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let overdue_task: Value = serde_json::from_slice(&body)?;
    assert_eq!(overdue_task["progress"], 100);
    assert!(overdue_task["completed_at"].is_string());
    assert_eq!(overdue_task["completed_at_is_backfilled"], false);
    assert_eq!(overdue_task["schedule_status"], "overdue");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?schedule_status=overdue",
                    project_id
                ))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let overdue_tasks: Value = serde_json::from_slice(&body)?;
    let overdue_tasks = overdue_tasks
        .as_array()
        .context("missing overdue tasks array")?;
    assert_eq!(overdue_tasks.len(), 1);
    assert_eq!(overdue_tasks[0]["id"], overdue_task_id);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?schedule_status=finished_early,not_specified",
                    project_id
                ))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let filtered_tasks: Value = serde_json::from_slice(&body)?;
    let filtered_tasks = filtered_tasks
        .as_array()
        .context("missing filtered tasks array")?;
    assert_eq!(filtered_tasks.len(), 3);
    assert!(filtered_tasks.iter().any(|task| task["id"] == task_id));
    assert!(filtered_tasks
        .iter()
        .any(|task| task["id"] == no_due_task_id));
    assert!(filtered_tasks
        .iter()
        .any(|task| task["id"] == legacy_task_id.to_string()));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks/{}", project_id, legacy_task_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let legacy_task: Value = serde_json::from_slice(&body)?;
    assert_eq!(legacy_task["completed_at"], legacy_completed_at);
    assert_eq!(legacy_task["completed_at_is_backfilled"], true);
    assert_eq!(legacy_task["schedule_status"], "finished_early");

    Ok(())
}
