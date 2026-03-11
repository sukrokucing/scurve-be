#![allow(clippy::uninlined_format_args)]

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use s_curve::create_app;
use serde_json::{json, Value};
use tower::ServiceExt;

mod support;

#[tokio::test]
async fn test_password_reset_flow() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("AUTH_RATE_PER_SECOND", "100");
    std::env::set_var("AUTH_BURST_SIZE", "100");
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
    let token = msg
        .split(": ")
        .nth(1)
        .context("failed to parse token from message")?
        .trim();
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
