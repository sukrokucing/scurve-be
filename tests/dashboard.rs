use anyhow::Context;
use anyhow::Result;
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::json;
use sqlx::SqlitePool;
use tower::util::ServiceExt; // for `oneshot`
use tempfile::tempdir;
use uuid::Uuid;
use chrono::Utc;

use s_curve::create_app;

#[tokio::test]
async fn project_dashboard_returns_plan_and_actual() -> Result<()> {
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
        "name": "Dash User",
        "email": "dash_user@example.com",
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

    // create project via API
    let project_body = json!({"name": "Dashboard Project"});
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
    let _project_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    // fetch the project id from the DB (avoid relying on JSON id format mismatch)
    let project_uuid: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM projects WHERE user_id = (SELECT id FROM users WHERE email = ?) ORDER BY created_at DESC LIMIT 1",
    )
    .bind("dash_user@example.com")
    .fetch_one(&pool)
    .await?;
    let project_id = project_uuid.to_string();

    // create a task via API
    let task_body = json!({"title": "Dashboard Task", "status": "pending"});
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

    // insert two planned points directly into project_plan
    let now = Utc::now();
    let p1_date = (now + chrono::Duration::days(1)).to_rfc3339();
    let p2_date = (now + chrono::Duration::days(7)).to_rfc3339();

    let pp1_uuid = Uuid::new_v4();
    let pp2_uuid = Uuid::new_v4();

    sqlx::query("INSERT INTO project_plan (id, project_id, date, planned_progress, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(pp1_uuid)
        .bind(project_uuid)
        .bind(&p1_date)
        .bind(10i32)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await?;

    sqlx::query("INSERT INTO project_plan (id, project_id, date, planned_progress, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(pp2_uuid)
        .bind(project_uuid)
        .bind(&p2_date)
        .bind(50i32)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await?;

    // create a progress entry via API (this will be included in actual aggregation)
    let prog_body = json!({"progress": 42, "note": "initial"});
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{}/tasks/{}/progress", project_id, task_id))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(prog_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let _body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!("progress create failed: {}", status);
    }

    // call dashboard endpoint
    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/dashboard", project_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!("dashboard request failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }

    let dash_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    // check structure
    assert!(dash_res.get("project").is_some());
    let plan = dash_res.get("plan").and_then(|v| v.as_array()).context("missing plan array")?;
    assert_eq!(plan.len(), 2);
    let actual = dash_res.get("actual").and_then(|v| v.as_array()).context("missing actual array")?;
    assert!(actual.len() >= 1);

    Ok(())
}
