use anyhow::Context;
use anyhow::Result;
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::json;
use sqlx::SqlitePool;
use tower::util::ServiceExt; // for `oneshot`
use tempfile::tempdir;
// uuid not needed in this test file

use s_curve::create_app;

#[tokio::test]
async fn get_single_task_and_progress_happy_path_and_404() -> Result<()> {
    // setup temp sqlite database and run migrations
    let dir = tempdir().context("failed to create tempdir")?;
    let db_path = dir.path().join("test.db");
    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::new()
        .filename(db_path.as_path())
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")).await?;
    migrator.run(&pool).await?;

    // create app router
    std::env::set_var("JWT_SECRET", "test-secret");
    let app = create_app(pool.clone()).await?;

    // register a user and obtain token
    let register_body = json!({
        "name": "Single Test User",
        "email": "single_test@example.com",
        "password": "password123"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(register_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!("register failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let auth_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let token = auth_res.get("token").and_then(|v| v.as_str()).context("missing token")?.to_string();
    let user_id = auth_res.get("user").and_then(|u| u.get("id"))
        .and_then(|v| v.as_str()).context("missing user id")?.to_string();
    // mark as intentionally unused for now
    let _user_id = user_id.clone();

    // create project via API
    let project_body = json!({"name": "Single Test Project"});
    let req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(project_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!("project create failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let project_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let project_id = project_res.get("id").and_then(|v| v.as_str()).context("missing project id")?.to_string();

    // create task via API
    let task_body = json!({"title": "Single Test Task", "status": "pending"});
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{}/tasks", project_id))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(task_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!("task create failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let task_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let task_id = task_res.get("id").and_then(|v| v.as_str()).context("missing task id")?.to_string();

    // create progress via API
    let prog_body = json!({"progress": 33, "note": "one third"});
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{}/tasks/{}/progress", project_id, task_id))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(prog_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!("progress create failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let prog_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let prog_id = prog_res.get("id").and_then(|v| v.as_str()).context("missing progress id")?.to_string();

    // --- Happy path: GET single task
    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!("get task failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let got_task: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let got_task_id = got_task.get("id").and_then(|v| v.as_str()).context("missing id")?;
    assert_eq!(got_task_id, task_id.as_str());

    // --- Happy path: GET single progress
    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/tasks/{}/progress/{}", project_id, task_id, prog_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!("get progress failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let got_prog: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let got_prog_id = got_prog.get("id").and_then(|v| v.as_str()).context("missing id")?;
    assert_eq!(got_prog_id, prog_id.as_str());

    // --- Delete progress and ensure GET now 404
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/projects/{}/tasks/{}/progress/{}", project_id, task_id, prog_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    if status != StatusCode::NO_CONTENT {
        let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
        panic!("progress delete failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }

    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/tasks/{}/progress/{}", project_id, task_id, prog_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // --- Delete task and ensure GET task now 404
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    if status != StatusCode::NO_CONTENT {
        let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
        panic!("task delete failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }

    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}
