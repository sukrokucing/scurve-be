use anyhow::Result;
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::Value;
use sqlx::SqlitePool;
use tempfile::tempdir;
use tower::util::ServiceExt; // for `oneshot`

use s_curve::create_app;

#[tokio::test]
async fn health_endpoint_reports_db_ok() -> Result<()> {
    // create temp dir and sqlite db
    let dir = tempdir()?;
    let db_path = dir.path().join("test.db");

    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::new()
        .filename(db_path.as_path())
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;

    // run migrations
    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")).await?;
    migrator.run(&pool).await?;

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
