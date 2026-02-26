use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt; // for oneshot
use uuid::Uuid;

use s_curve::create_app;
use s_curve::jwt::JwtConfig;
use s_curve::authz::permissions;

mod support;

async fn setup_app() -> Result<(axum::Router, SqlitePool, String, Uuid, support::db::TestDb)> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    std::env::set_var("JWT_SECRET", "test-secret-rbac-admin");
     // Ensure strict mode is set so we get 403s
    std::env::set_var("AUTHZ_MODE", "strict");

    let app = create_app(pool.clone()).await?;

    // Create Admin User
    let admin_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Admin', 'admin@example.com', 'hash', 'local', ?, ?)")
        .bind(admin_id.to_string())
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    // Grant all necessary permissions for RBAC management
    let perms = vec![
        permissions::ROLE_VIEW,
        permissions::ROLE_MANAGE,
        permissions::PERMISSION_VIEW,
        permissions::PERMISSION_MANAGE,
        permissions::USER_VIEW,
        permissions::USER_MANAGE,
        // Needed for POST /rbac/users/{user_id}/roles in strict authz mode.
    ];

    for perm_name in perms {
        let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
            .bind(perm_name)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| panic!("Permission {} not found in seeds", perm_name));

        sqlx::query("INSERT INTO user_permissions (id, user_id, permission_id, created_at) VALUES (?, ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(admin_id.to_string())
            .bind(perm_id)
            .bind(now)
            .execute(&pool)
            .await?;
    }

    let jwt_config = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret-rbac-admin".to_vec()),
        exp_hours: 1,
    };
    let token = jwt_config.encode(admin_id)?;

    Ok((app, pool, token, admin_id, test_db))
}

#[tokio::test]
async fn test_rbac_admin_lifecycle() -> Result<()> {
    let (app, pool, token, _admin_id, _test_db) = setup_app().await?;

    // 1. List Roles (expect seeds)
    let req = Request::builder()
        .method("GET")
        .uri("/rbac/roles")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await?)?;
    let roles = body.as_array().context("expected array")?;
    assert!(roles.len() >= 1, "Should have seeded roles");

    // 2. Create New Role
    let new_role_body = json!({
        "name": "Custom Role",
        "description": "A custom test role"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/rbac/roles")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(new_role_body.to_string()))?;
    let resp = app.clone().oneshot(req).await?;
    if resp.status() != StatusCode::CREATED {
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await?;
        let body_str = String::from_utf8_lossy(&body);
        panic!("Failed to create role: Status: {}, Body: {}", status, body_str);
    }
    let created_role: Value = serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await?)?;
    let role_id = created_role["id"].as_str().context("missing id")?;

    // 3. Assign Permission to Role
    // Get PROJECT_VIEW permission id
    let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
        .bind(permissions::PROJECT_VIEW)
        .fetch_one(&pool)
        .await?;

    let assign_body = json!({ "permission_id": perm_id });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/rbac/roles/{}/permissions", role_id))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(assign_body.to_string()))?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);
    // Wait, create usually returns 201. Assign usually returns 200 or 201.
    // Checked rbac.rs: assign_permission_to_role returns matching status?
    // Usually standard is 201 Created or 200 OK. Let's inspect response if it fails.

    // 4. Create a target user
    let target_user_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Target', 'target@example.com', 'hash', 'local', ?, ?)")
        .bind(target_user_id.to_string())
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    // 5. Assign Role to User
    let assign_role_body = json!({ "role_id": role_id });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/rbac/users/{}/roles", target_user_id))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(assign_role_body.to_string()))?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 6. Verify Effective Permissions for Target User
    // Verify specific endpoint? GET /users/:id/effective-permissions
    // Wait, that endpoint requires USER_VIEW. Admin has it.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/rbac/users/{}/effective-permissions", target_user_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())?;
    let resp = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), usize::MAX).await?)?;
    let perms = body["permissions"].as_array().context("expected permissions array")?;

    // Should have PROJECT_VIEW
    let has_perm = perms.iter().any(|p| p["name"].as_str() == Some(permissions::PROJECT_VIEW));
    if !has_perm {
        println!("Effective Permissions Response: {}", serde_json::to_string_pretty(&body).unwrap());
    }
    assert!(has_perm, "Target user should have inherited PROJECT_VIEW from Custom Role");

    Ok(())
}
