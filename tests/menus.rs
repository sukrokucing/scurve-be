//! Integration tests for GET /menus (RBAC-filtered navigation menus).
//!
//! NOTE: `cloned_clean_db()` truncates `user_permissions` and `role_permissions` between
//! tests. The `menus`, `menu_version`, and `permissions` tables are NOT cleared (static
//! seed data). Each test must explicitly grant `menu.view` to the test user — without it
//! the authz middleware returns 403 before the handler runs.

#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use s_curve::create_app;
use s_curve::jwt::JwtConfig;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

const JWT_SECRET: &str = "test-secret-menus";

// ---------------------------------------------------------------------------
// Setup helpers (mirrors view_as_and_permissions.rs conventions)
// ---------------------------------------------------------------------------

async fn setup() -> Result<(axum::Router, SqlitePool, support::db::TestDb)> {
    std::env::set_var("JWT_SECRET", JWT_SECRET);
    std::env::set_var("AUTHZ_MODE", "strict");

    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();
    let app = create_app(pool.clone()).await?;

    Ok((app, pool, test_db))
}

fn make_token(user_id: Uuid) -> String {
    let jwt = JwtConfig {
        secret: Arc::new(JWT_SECRET.as_bytes().to_vec()),
        exp_hours: 1,
    };
    jwt.encode(user_id).unwrap()
}

async fn insert_user(pool: &SqlitePool, label: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at)
         VALUES (?, ?, ?, 'hash', 'local', ?, ?)",
    )
    .bind(user_id.to_string())
    .bind(label)
    .bind(format!(
        "{}-{}@test.com",
        label.replace(' ', "-"),
        &user_id.to_string()[..8]
    ))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    user_id
}

/// Grant permissions directly to a user (bypasses role_permissions which gets cleared).
async fn grant_user_permissions(pool: &SqlitePool, user_id: Uuid, perm_names: &[&str]) {
    let now = chrono::Utc::now();
    for perm_name in perm_names {
        let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
            .bind(perm_name)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| panic!("seeded permission '{}' not found", perm_name));

        sqlx::query(
            "INSERT INTO user_permissions (id, user_id, permission_id, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id.to_string())
        .bind(perm_id)
        .bind(now)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// GET /menus with token and optional extra headers.
/// Returns the full response so callers can inspect status + headers.
async fn get_menus(
    app: &axum::Router,
    token: &str,
    extra_headers: &[(&str, &str)],
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("GET")
        .uri("/menus")
        .header("Authorization", format!("Bearer {}", token));

    for (k, v) in extra_headers {
        builder = builder.header(*k, *v);
    }

    let req = builder.body(Body::empty()).unwrap();
    app.clone().oneshot(req).await.unwrap()
}

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), 1_000_000)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn menu_ids(body: &Value) -> Vec<&str> {
    body["menus"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Unauthenticated request is rejected before reaching the handler.
#[tokio::test]
async fn menus_requires_authentication() -> Result<()> {
    let (app, _pool, _db) = setup().await?;

    let req = Request::builder()
        .method("GET")
        .uri("/menus")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

/// A user with only `menu.view` sees ungated menus (dashboard, settings)
/// but NOT menus gated on other permissions.
#[tokio::test]
async fn menus_hides_permission_gated_items_by_default() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let user_id = insert_user(&pool, "basic-user").await;
    // menu.view is required to reach the endpoint at all.
    // No project.view / task.view / user.manage / role.manage.
    grant_user_permissions(&pool, user_id, &["menu.view"]).await;
    let token = make_token(user_id);

    let res = get_menus(&app, &token, &[]).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = body_json(res).await;
    let ids = menu_ids(&body);

    // Ungated menus (required_permission IS NULL) always visible.
    assert!(ids.contains(&"dashboard"), "dashboard (no gate) must appear, got: {:?}", ids);
    assert!(ids.contains(&"settings"), "settings (no gate) must appear, got: {:?}", ids);

    // Gated menus must be absent.
    assert!(!ids.contains(&"projects"), "projects requires project.view, must be hidden");
    assert!(!ids.contains(&"tasks"), "tasks requires task.view, must be hidden");
    assert!(!ids.contains(&"settings-users"), "settings-users requires user.manage");
    assert!(!ids.contains(&"settings-roles"), "settings-roles requires role.manage");
    assert!(!ids.contains(&"settings-policy"), "settings-policy requires role.manage");
    assert!(!ids.contains(&"settings-flow"), "settings-flow requires user.manage");

    Ok(())
}

/// Adding project.view unlocks the Projects menu (and only that).
#[tokio::test]
async fn menus_shows_projects_when_user_has_project_view() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let user_id = insert_user(&pool, "viewer").await;
    grant_user_permissions(&pool, user_id, &["menu.view", "project.view"]).await;
    let token = make_token(user_id);

    let res = get_menus(&app, &token, &[]).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = body_json(res).await;
    let ids = menu_ids(&body);

    assert!(ids.contains(&"dashboard"));
    assert!(ids.contains(&"settings"));
    assert!(ids.contains(&"projects"), "project.view should unlock Projects menu");
    assert!(!ids.contains(&"tasks"), "still no task.view");
    assert!(!ids.contains(&"settings-users"), "still no user.manage");

    Ok(())
}

/// A fully-privileged user sees all 8 menus.
#[tokio::test]
async fn menus_admin_sees_all_menus() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let admin_id = insert_user(&pool, "admin").await;
    grant_user_permissions(
        &pool,
        admin_id,
        &[
            "menu.view",
            "project.view",
            "task.view",
            "user.manage",
            "role.manage",
        ],
    )
    .await;
    let token = make_token(admin_id);

    let res = get_menus(&app, &token, &[]).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = body_json(res).await;
    let menus = body["menus"].as_array().unwrap();
    assert_eq!(menus.len(), 8, "fully-privileged user should see all 8 menus, got: {:?}", menus);

    let ids: Vec<&str> = menus.iter().map(|m| m["id"].as_str().unwrap()).collect();
    for expected in &[
        "dashboard",
        "projects",
        "tasks",
        "settings",
        "settings-users",
        "settings-roles",
        "settings-policy",
        "settings-flow",
    ] {
        assert!(ids.contains(expected), "missing menu: {}", expected);
    }

    Ok(())
}

/// Response structure: version field present, ETag + Cache-Control headers set.
#[tokio::test]
async fn menus_response_has_version_and_cache_headers() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let user_id = insert_user(&pool, "cache-test-user").await;
    grant_user_permissions(&pool, user_id, &["menu.view"]).await;
    let token = make_token(user_id);

    let res = get_menus(&app, &token, &[]).await;
    assert_eq!(res.status(), StatusCode::OK);

    let etag = res.headers().get("etag");
    let cache_control = res.headers().get("cache-control");
    assert!(etag.is_some(), "ETag header must be present");
    assert!(cache_control.is_some(), "Cache-Control header must be present");

    let cc = cache_control.unwrap().to_str().unwrap();
    assert!(cc.contains("private"), "Cache-Control must be private");
    assert!(cc.contains("max-age=300"), "Cache-Control must include max-age=300");

    let body = body_json(res).await;
    assert_eq!(
        body["version"].as_i64(),
        Some(1),
        "initial menu version should be 1"
    );

    Ok(())
}

/// Sending If-None-Match with the current ETag yields 304 with no body.
#[tokio::test]
async fn menus_returns_304_on_matching_etag() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let user_id = insert_user(&pool, "etag-user").await;
    grant_user_permissions(&pool, user_id, &["menu.view"]).await;
    let token = make_token(user_id);

    // First request — capture ETag.
    let res1 = get_menus(&app, &token, &[]).await;
    assert_eq!(res1.status(), StatusCode::OK);
    let etag = res1
        .headers()
        .get("etag")
        .expect("ETag must be present")
        .to_str()
        .unwrap()
        .to_string();

    // Second request — send matching If-None-Match.
    let res2 = get_menus(&app, &token, &[("if-none-match", etag.as_str())]).await;
    assert_eq!(
        res2.status(),
        StatusCode::NOT_MODIFIED,
        "matching ETag should yield 304"
    );

    // 304 must have no body.
    let body_bytes = axum::body::to_bytes(res2.into_body(), 1_000_000)
        .await
        .unwrap();
    assert!(body_bytes.is_empty(), "304 response must have empty body");

    Ok(())
}

/// ETag changes when permissions change (different permission fingerprint).
#[tokio::test]
async fn menus_etag_differs_for_different_permission_sets() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // User A: only menu.view
    let user_a = insert_user(&pool, "user-a").await;
    grant_user_permissions(&pool, user_a, &["menu.view"]).await;

    // User B: menu.view + project.view
    let user_b = insert_user(&pool, "user-b").await;
    grant_user_permissions(&pool, user_b, &["menu.view", "project.view"]).await;

    let token_a = make_token(user_a);
    let token_b = make_token(user_b);

    let res_a = get_menus(&app, &token_a, &[]).await;
    let res_b = get_menus(&app, &token_b, &[]).await;

    let etag_a = res_a.headers().get("etag").unwrap().to_str().unwrap().to_string();
    let etag_b = res_b.headers().get("etag").unwrap().to_str().unwrap().to_string();

    assert_ne!(
        etag_a, etag_b,
        "ETags must differ when permission sets differ (got: {} vs {})",
        etag_a, etag_b
    );

    Ok(())
}

/// Menus are ordered by section then priority within each section.
#[tokio::test]
async fn menus_are_returned_in_priority_order() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let user_id = insert_user(&pool, "order-user").await;
    // Grant all permissions to see all menus.
    grant_user_permissions(
        &pool,
        user_id,
        &["menu.view", "project.view", "task.view", "user.manage", "role.manage"],
    )
    .await;
    let token = make_token(user_id);

    let res = get_menus(&app, &token, &[]).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = body_json(res).await;
    let menus = body["menus"].as_array().unwrap();

    // Extract (section, priority) pairs and verify they are non-decreasing within section.
    let pairs: Vec<(&str, i64)> = menus
        .iter()
        .map(|m| {
            (
                m["section"].as_str().unwrap(),
                m["priority"].as_i64().unwrap(),
            )
        })
        .collect();

    let mut prev_section = "";
    let mut prev_priority = i64::MIN;
    for (section, priority) in &pairs {
        if *section != prev_section {
            prev_section = section;
            prev_priority = i64::MIN;
        }
        assert!(
            *priority >= prev_priority,
            "menus must be ordered by priority within section (got {} after {} in section {})",
            priority,
            prev_priority,
            section
        );
        prev_priority = *priority;
    }

    Ok(())
}

/// Surfaces and keywords are deserialized into arrays in the JSON response (not raw strings).
#[tokio::test]
async fn menus_surfaces_and_keywords_are_arrays() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let user_id = insert_user(&pool, "array-check-user").await;
    grant_user_permissions(&pool, user_id, &["menu.view"]).await;
    let token = make_token(user_id);

    let res = get_menus(&app, &token, &[]).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = body_json(res).await;
    let dashboard = body["menus"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "dashboard")
        .expect("dashboard must be present");

    assert!(
        dashboard["surfaces"].is_array(),
        "surfaces must be a JSON array, got: {}",
        dashboard["surfaces"]
    );
    assert!(
        dashboard["keywords"].is_array(),
        "keywords must be a JSON array, got: {}",
        dashboard["keywords"]
    );
    assert!(
        dashboard["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "sidebar"),
        "dashboard surfaces must include 'sidebar'"
    );

    Ok(())
}
