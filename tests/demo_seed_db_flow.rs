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

use s_curve::create_app;

#[tokio::test]
async fn seeded_db_is_visible_via_api() -> Result<()> {
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

    // register a user to obtain an auth token and user_id
    let register_body = json!({
        "name": "Seeded User",
        "email": "seeded@example.com",
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
    let user_id = auth_res.get("user").and_then(|u| u.get("id")).and_then(|v| v.as_str()).context("missing user id")?.to_string();

    // seed a project, task and two progress rows directly into the DB using SQL
    let project_uuid = Uuid::new_v4();
    let task_uuid = Uuid::new_v4();
    let prog1_uuid = Uuid::new_v4();
    let prog2_uuid = Uuid::new_v4();
    let project_id = project_uuid.to_string();
    let task_id = task_uuid.to_string();
    let prog1_id = prog1_uuid.to_string();
    let prog2_id = prog2_uuid.to_string();

    // insert project with ISO8601 timestamps to match application format
    let user_uuid = uuid::Uuid::parse_str(&user_id)?;
    let now = chrono::Utc::now();
    sqlx::query("INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(project_uuid)
        .bind(user_uuid)
        .bind("Seeded Project")
        .bind("project seeded by test")
        .bind("#112233")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    // insert task
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task_uuid)
        .bind(project_uuid)
        .bind("Seeded Task")
        .bind("pending")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    // insert two progress rows
    sqlx::query("INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(prog1_uuid)
        .bind(task_uuid)
        .bind(project_uuid)
        .bind(5_i64)
        .bind("seeded p1")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    sqlx::query("INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(prog2_uuid)
        .bind(task_uuid)
        .bind(project_uuid)
        .bind(75_i64)
        .bind("seeded p2")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    // -- now call API endpoints with the token and verify seeded rows are visible
    // GET /projects and ensure our project appears
    let req = Request::builder()
        .method("GET")
        .uri("/projects")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!("projects list failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let projects: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    assert!(projects.as_array().unwrap().iter().any(|p| p.get("id").and_then(|id| id.as_str()) == Some(&project_id)));

    // GET /projects/{project_id}/tasks and ensure our task appears
    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/tasks", project_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!("tasks list failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let tasks: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    assert!(tasks.as_array().unwrap().iter().any(|t| t.get("id").and_then(|id| id.as_str()) == Some(&task_id)));

    // GET /projects/{project_id}/tasks/{task_id}/progress and ensure seeded progress rows exist
    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}/tasks/{}/progress", project_id, task_id))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!("progress list failed: {} - {}", status, String::from_utf8_lossy(&body_bytes));
    }
    let progress_list: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let ids: Vec<&str> = progress_list.as_array().unwrap().iter().filter_map(|v| v.get("id").and_then(|x| x.as_str())).collect();
    assert!(ids.contains(&prog1_id.as_str()));
    assert!(ids.contains(&prog2_id.as_str()));

    Ok(())
}
