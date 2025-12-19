use anyhow::{Context, Result};
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::json;
use sqlx::SqlitePool;
use tower::util::ServiceExt;
use tempfile::tempdir;

use s_curve::create_app;

#[tokio::test]
async fn test_list_users_pagination_and_search() -> Result<()> {
    // Setup test environment
    let dir = tempdir().context("failed to create tempdir")?;
    let db_path = dir.path().join("test.db");
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

    // 1. Register a test user
    let register_body = json!({
        "name": "Alice User",
        "email": "alice@example.com",
        "password": "password123"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(register_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    let auth_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let token = auth_res.get("token").and_then(|v| v.as_str()).context("missing token")?.to_string();

    // 2. Register another user
    let register_body2 = json!({
        "name": "Bob Admin",
        "email": "bob@example.com",
        "password": "password123"
    });

    let req2 = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(register_body2.to_string()))?;

    let resp2: Response = app.clone().oneshot(req2).await?;
    assert_eq!(resp2.status(), StatusCode::CREATED);

    // 3. List all users (should have at least 2)
    let req_list = Request::builder()
        .method("GET")
        .uri("/users")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_list: Response = app.clone().oneshot(req_list).await?;
    assert_eq!(resp_list.status(), StatusCode::OK);

    let total_count = resp_list.headers().get("X-Total-Count").unwrap().to_str().unwrap().parse::<i64>().unwrap();
    assert!(total_count >= 2);

    let body_bytes = body::to_bytes(resp_list.into_body(), 10_485_760).await?;
    let users: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes)?;
    assert!(users.len() >= 2);

    // 4. Test search
    let req_search = Request::builder()
        .method("GET")
        .uri("/users?q=Bob")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_search: Response = app.clone().oneshot(req_search).await?;
    assert_eq!(resp_search.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(resp_search.into_body(), 10_485_760).await?;
    let results: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "Bob Admin");

    // 5. Test pagination
    let req_pag = Request::builder()
        .method("GET")
        .uri("/users?per_page=1")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_pag: Response = app.clone().oneshot(req_pag).await?;
    assert_eq!(resp_pag.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(resp_pag.into_body(), 10_485_760).await?;
    let pag_results: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes)?;
    assert_eq!(pag_results.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_list_users_unauthorized() -> Result<()> {
    let dir = tempdir().context("failed to create tempdir")?;
    let db_path = dir.path().join("test.db");
    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::new()
        .filename(db_path.as_path())
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;

    std::env::set_var("JWT_SECRET", "test-secret");
    let app = create_app(pool.clone()).await?;

    let req = Request::builder()
        .method("GET")
        .uri("/users")
        .body(Body::empty())?;

    let resp: Response = app.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}
