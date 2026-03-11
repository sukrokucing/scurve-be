#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use s_curve::create_app;
use serde_json::json;
use tower::util::ServiceExt;

mod support;

#[tokio::test]
async fn auth_edge_cases() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

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
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Should fail with bad request for short password"
    );

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
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Should fail with unauthorized for wrong password"
    );

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
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Should fail with unauthorized for non-existent user"
    );

    // 5. Access protected route without token
    let req = Request::builder()
        .method("GET")
        .uri("/projects")
        .body(Body::empty())?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Should fail with unauthorized for missing token"
    );

    Ok(())
}
