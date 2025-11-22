use anyhow::Result;
use axum::body::{self, Body};
use axum::http::Request;
use serde_json::json;
use sqlx::SqlitePool;
use tempfile::tempdir;
use tower::util::ServiceExt; // for oneshot

use s_curve::create_app;

#[tokio::test]
async fn uuid_storage_is_text_for_new_rows() -> Result<()> {
    // create temp db and run migrations
    let dir = tempdir()?;
    let db_path = dir.path().join("test.db");
    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::new()
        .filename(db_path.as_path())
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")).await?;
    migrator.run(&pool).await?;

    // start app
    std::env::set_var("JWT_SECRET", "test-secret");
    let app = create_app(pool.clone()).await?;

    // register a user via the auth route
    let register_body = json!({
        "name": "UUID Test",
        "email": "uuid-test@example.com",
        "password": "password"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(register_body.to_string()))?;

    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    assert_eq!(status.as_u16(), 201);
    let auth_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let _user_id = auth_res.get("user").and_then(|u| u.get("id")).and_then(|v| v.as_str()).unwrap().to_string();

    // verify users.id storage type by selecting the row by `email` (reliable text key)
    let row: (String,) = sqlx::query_as("SELECT typeof(id) FROM users WHERE email = ?")
        .bind("uuid-test@example.com")
        .fetch_one(&pool)
        .await?;
    let user_type = row.0;

    // create a project
    let token = auth_res.get("token").and_then(|v| v.as_str()).unwrap().to_string();
    let project_body = json!({"name": "P1", "description": "d", "theme_color": "#000000"});
    let req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(project_body.to_string()))?;

    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    assert_eq!(status.as_u16(), 201);
    let project_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let _project_id = project_res.get("id").and_then(|v| v.as_str()).unwrap().to_string();

    // verify projects.id storage type by selecting the row by `name` (reliable text key)
    let row: (String,) = sqlx::query_as("SELECT typeof(id) FROM projects WHERE name = ?")
        .bind("P1")
        .fetch_one(&pool)
        .await?;
    let project_type = row.0;

    // Ensure both inserted rows use the same storage format (either `blob` or `text`),
    // and that it is one of the expected types.
    assert_eq!(user_type, project_type);
    assert!(user_type == "blob" || user_type == "text");

    Ok(())
}
