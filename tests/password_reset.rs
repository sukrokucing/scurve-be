use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use tempfile::tempdir;
use s_curve::create_app;

#[tokio::test]
async fn test_password_reset_flow() -> Result<()> {
    // 1. Setup Test Environment
    let dir = tempdir().context("failed to create tempdir")?;
    let db_path = dir.path().join("test_reset.db");
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

    // 2. Register User
    let email = "reset@example.com";
    let old_pass = "OldPass123";
    let register_body = json!({
        "name": "Reset User",
        "email": email,
        "password": old_pass
    });

    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3. Request Password Reset Link
    let forgot_body = json!({ "email": email });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/forgot-password")
        .header("Content-Type", "application/json")
        .body(Body::from(forgot_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await?;
    let msg_json: Value = serde_json::from_slice(&body_bytes)?;
    let msg = msg_json["message"].as_str().context("missing message")?;

    // Parse token from "Reset token (dev only): <token>"
    let token = msg.split(": ").nth(1).context("failed to parse token from message")?.trim();
    println!("Got reset token: {}", token);

    // 4. Use Token to Reset Password
    let new_pass = "NewSecurePass123!";
    let reset_body = json!({
        "token": token,
        "new_password": new_pass
    });

    let req = Request::builder()
        .method("POST")
        .uri("/auth/reset-password")
        .header("Content-Type", "application/json")
        .body(Body::from(reset_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Verify Old Password Fails
    let login_old = json!({ "email": email, "password": old_pass });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(login_old.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 6. Verify New Password Succeeds
    let login_new = json!({ "email": email, "password": new_pass });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(login_new.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // 7. Verify Token Cannot Be Reused
    let req = Request::builder()
        .method("POST")
        .uri("/auth/reset-password")
        .header("Content-Type", "application/json")
        .body(Body::from(reset_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    Ok(())
}
