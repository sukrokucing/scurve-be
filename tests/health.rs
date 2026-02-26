use anyhow::Result;
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::Value;
use tower::util::ServiceExt; // for `oneshot`

use s_curve::create_app;

mod support;

#[tokio::test]
async fn health_endpoint_reports_db_ok() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // create app router
    std::env::set_var("JWT_SECRET", "test-secret");
    let app = create_app(pool.clone()).await?;

    let req = Request::builder()
        .method("GET")
        .uri("/api/health")
        .body(Body::empty())?;

    let resp: Response = app.oneshot(req).await?;
    let status = resp.status();
    assert_eq!(status, StatusCode::OK, "health endpoint did not return 200");

    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    let v: Value = serde_json::from_slice(&body_bytes)?;
    let db_ok = v.get("db_ok").and_then(|b| b.as_bool()).unwrap_or(false);
    assert!(db_ok, "expected db_ok: true, got: {}", v);

    Ok(())
}
