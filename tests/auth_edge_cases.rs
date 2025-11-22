use anyhow::Context;
use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::json;
use sqlx::SqlitePool;
use tower::util::ServiceExt;
use tempfile::tempdir;
use s_curve::create_app;

#[tokio::test]
async fn auth_edge_cases() -> Result<()> {
    let dir = tempdir().context("failed to create tempdir")?;
    let db_path = dir.path().join("test_auth.db");
    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::new()
        .filename(db_path.as_path())
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"))
        .await?;
    migrator.run(&pool).await?;

    std::env::set_var("JWT_SECRET", "test-secret");
    let app = create_app(pool.clone()).await?;

    // 1. Register with short password
    let short_pass_body = json!({
        "name": "Short Pass",
        "email": "short@example.com",
        "password": "short"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(short_pass_body.to_string()))?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Should fail with bad request for short password");

    // 2. Register with valid user
    let valid_body = json!({
        "name": "Valid User",
        "email": "valid@example.com",
        "password": "password123"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(valid_body.to_string()))?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3. Login with wrong password
    let wrong_pass_body = json!({
        "email": "valid@example.com",
        "password": "wrongpassword"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(wrong_pass_body.to_string()))?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "Should fail with unauthorized for wrong password");

    // 4. Login with non-existent email
    let no_user_body = json!({
        "email": "nobody@example.com",
        "password": "password123"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(no_user_body.to_string()))?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "Should fail with unauthorized for non-existent user");

    // 5. Access protected route without token
    let req = Request::builder()
        .method("GET")
        .uri("/projects")
        .body(Body::empty())?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "Should fail with unauthorized for missing token");

    Ok(())
}
