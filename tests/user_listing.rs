#![allow(clippy::uninlined_format_args)]

use anyhow::{Context, Result};
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::json;
use tower::util::ServiceExt;

use s_curve::create_app;

mod support;

#[tokio::test]
async fn test_list_users_pagination_and_search() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

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
    let token = auth_res
        .get("token")
        .and_then(|v| v.as_str())
        .context("missing token")?
        .to_string();

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

    // 2b. Register a user containing SQL-LIKE wildcard characters in name.
    let register_body3 = json!({
        "name": "Percent 100% User",
        "email": "percent@example.com",
        "password": "password123"
    });

    let req3 = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(register_body3.to_string()))?;

    let resp3: Response = app.clone().oneshot(req3).await?;
    assert_eq!(resp3.status(), StatusCode::CREATED);

    // 3. List all users (should have at least 2)
    let req_list = Request::builder()
        .method("GET")
        .uri("/users")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_list: Response = app.clone().oneshot(req_list).await?;
    assert_eq!(resp_list.status(), StatusCode::OK);

    let total_count = resp_list
        .headers()
        .get("X-Total-Count")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
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

    // 4b. Search should trim whitespace.
    let req_search_trimmed = Request::builder()
        .method("GET")
        .uri("/users?q=%20%20Bob%20%20")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_search_trimmed: Response = app.clone().oneshot(req_search_trimmed).await?;
    assert_eq!(resp_search_trimmed.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(resp_search_trimmed.into_body(), 10_485_760).await?;
    let trimmed_results: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes)?;
    assert_eq!(trimmed_results.len(), 1);
    assert_eq!(trimmed_results[0]["name"], "Bob Admin");

    // 4c. Wildcard characters are treated literally, not as "match all".
    let req_search_percent = Request::builder()
        .method("GET")
        .uri("/users?q=%25")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_search_percent: Response = app.clone().oneshot(req_search_percent).await?;
    assert_eq!(resp_search_percent.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(resp_search_percent.into_body(), 10_485_760).await?;
    let percent_results: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes)?;
    assert_eq!(percent_results.len(), 1);
    assert_eq!(percent_results[0]["name"], "Percent 100% User");

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

    // 5b. per_page below minimum is clamped to 1.
    let req_pag_zero = Request::builder()
        .method("GET")
        .uri("/users?per_page=0")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp_pag_zero: Response = app.clone().oneshot(req_pag_zero).await?;
    assert_eq!(resp_pag_zero.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(resp_pag_zero.into_body(), 10_485_760).await?;
    let pag_zero_results: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes)?;
    assert_eq!(pag_zero_results.len(), 1);

    // 6. Excessive query length should be rejected.
    let long_q = "a".repeat(129);
    let req_too_long = Request::builder()
        .method("GET")
        .uri(format!("/users?q={}", long_q))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;
    let resp_too_long: Response = app.clone().oneshot(req_too_long).await?;
    assert_eq!(resp_too_long.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_list_users_unauthorized() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

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
