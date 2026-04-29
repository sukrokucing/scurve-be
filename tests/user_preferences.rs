#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use s_curve::create_app;
use serde_json::{json, Value};
use tower::ServiceExt;

mod support;

async fn register_and_login(app: &axum::Router, email: &str) -> Result<String> {
    let body = json!({ "name": "Pref User", "email": email, "password": "Pass1234" });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))?;
    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await?;
    let v: Value = serde_json::from_slice(&body_bytes)?;
    Ok(v["token"].as_str().unwrap().to_string())
}

fn setup_env() {
    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("JWT_EXP_HOURS", "24");
    std::env::set_var("AUTH_RATE_PER_SECOND", "100");
    std::env::set_var("AUTH_BURST_SIZE", "100");
    std::env::set_var("GLOBAL_RATE_PER_SECOND", "1000");
    std::env::set_var("GLOBAL_BURST_SIZE", "2000");
    std::env::set_var("AUTHZ_MODE", "off");
}

async fn put_prefs(app: &axum::Router, token: &str, body: Value) -> Result<Response> {
    let req = Request::builder()
        .method("PUT")
        .uri("/auth/me/preferences")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))?;
    Ok(app.clone().oneshot(req).await?)
}

#[tokio::test]
async fn get_preferences_returns_defaults_when_unset() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-default@example.com").await?;

    let req = Request::builder()
        .method("GET")
        .uri("/auth/me/preferences")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await?)?;
    assert_eq!(body["timezone"], "Asia/Jakarta");
    assert_eq!(body["locale"], "id-ID");
    assert_eq!(body["hour_cycle"], 24);
    assert_eq!(body["currency"], "IDR");
    assert!(body["updated_at"].is_string());
    Ok(())
}

#[tokio::test]
async fn put_preferences_updates_and_me_embeds() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-put@example.com").await?;

    let resp = put_prefs(
        &app,
        &token,
        json!({
            "timezone": "Asia/Makassar",
            "locale": "en-US",
            "hour_cycle": 12,
            "currency": "USD"
        }),
    )
    .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await?)?;
    assert_eq!(body["timezone"], "Asia/Makassar");
    assert_eq!(body["locale"], "en-US");
    assert_eq!(body["hour_cycle"], 12);
    assert_eq!(body["currency"], "USD");

    let req = Request::builder()
        .method("GET")
        .uri("/auth/me")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await?)?;
    assert_eq!(body["preferences"]["timezone"], "Asia/Makassar");
    assert_eq!(body["preferences"]["locale"], "en-US");
    assert_eq!(body["preferences"]["hour_cycle"], 12);
    assert_eq!(body["preferences"]["currency"], "USD");
    Ok(())
}

#[tokio::test]
async fn put_preferences_normalizes_currency_case() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-normcur@example.com").await?;

    let resp = put_prefs(
        &app,
        &token,
        json!({
            "timezone": "Asia/Jakarta",
            "locale": "id-ID",
            "hour_cycle": 24,
            "currency": "usd"
        }),
    )
    .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await?)?;
    assert_eq!(body["currency"], "USD");
    Ok(())
}

#[tokio::test]
async fn put_preferences_rejects_invalid_currency() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-badcur@example.com").await?;

    let resp = put_prefs(
        &app,
        &token,
        json!({
            "timezone": "Asia/Jakarta",
            "locale": "id-ID",
            "hour_cycle": 24,
            "currency": "XYZ"
        }),
    )
    .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await?)?;
    let msg = body["message"].as_str().unwrap_or("");
    assert!(msg.contains("(field: currency)"), "got: {}", msg);
    Ok(())
}

#[tokio::test]
async fn put_preferences_rejects_missing_currency() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-nocur@example.com").await?;

    let resp = put_prefs(
        &app,
        &token,
        json!({
            "timezone": "Asia/Jakarta",
            "locale": "id-ID",
            "hour_cycle": 24
        }),
    )
    .await?;
    // serde rejects missing required field — axum turns it into 4xx
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn put_preferences_rejects_invalid_timezone() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-badtz@example.com").await?;

    let resp = put_prefs(
        &app,
        &token,
        json!({
            "timezone": "Mars/Olympus",
            "locale": "id-ID",
            "hour_cycle": 24,
            "currency": "USD"
        }),
    )
    .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn put_preferences_rejects_invalid_hour_cycle() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let token = register_and_login(&app, "prefs-badhc@example.com").await?;

    let resp = put_prefs(
        &app,
        &token,
        json!({
            "timezone": "Asia/Jakarta",
            "locale": "id-ID",
            "hour_cycle": 13,
            "currency": "IDR"
        }),
    )
    .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn login_embeds_preferences() -> Result<()> {
    setup_env();
    let test_db = support::db::cloned_clean_db().await?;
    let app = create_app(test_db.pool.clone()).await?;
    let email = "prefs-login@example.com";
    let _ = register_and_login(&app, email).await?;

    let body = json!({ "email": email, "password": "Pass1234" });
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let v: Value =
        serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await?)?;
    assert_eq!(v["user"]["preferences"]["timezone"], "Asia/Jakarta");
    assert_eq!(v["user"]["preferences"]["locale"], "id-ID");
    assert_eq!(v["user"]["preferences"]["hour_cycle"], 24);
    assert_eq!(v["user"]["preferences"]["currency"], "IDR");
    Ok(())
}
