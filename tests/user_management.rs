use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;
use tempfile::tempdir;

use s_curve::create_app;
use s_curve::jwt::JwtConfig;

#[tokio::test]
async fn test_user_management_crud() -> Result<()> {
    // 1. Setup Test Environment
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

    // 2. Create Admin User
    let admin_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Admin', 'admin@example.com', 'hash', 'local', ?, ?)")
        .bind(admin_id.to_string())
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await?;

    // 3. Grant 'user.manage' permission
    // Fetch the permission ID for 'user.manage' (seeded by migration)
    let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = 'user.manage'")
        .fetch_one(&pool)
        .await
        .context("user.manage permission not found")?;

    let perm_grant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO user_permissions (id, user_id, permission_id, created_at) VALUES (?, ?, ?, ?)")
        .bind(perm_grant_id.to_string())
        .bind(admin_id.to_string())
        .bind(perm_id)
        .bind(now)
        .execute(&pool)
        .await?;

    // 4. Generate JWT
    let jwt_config = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret".to_vec()),
        exp_hours: 1,
    };
    let token = jwt_config.encode(admin_id)?;

    // 5. Test CREATE User
    let new_user_body = json!({
        "name": "New User",
        "email": "new@example.com",
        "password": "Password123!"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(new_user_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await?;
    let created_user: Value = serde_json::from_slice(&body_bytes)?;
    let new_user_id_str = created_user["id"].as_str().context("missing id")?;
    let new_user_id = Uuid::parse_str(new_user_id_str)?;

    assert_eq!(created_user["name"], "New User");
    assert_eq!(created_user["email"], "new@example.com");

    // 6. Test UPDATE User
    let update_body = json!({
        "name": "Updated User",
        "email": "updated@example.com"
    });

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/users/{}", new_user_id))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(update_body.to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await?;
    let updated_user: Value = serde_json::from_slice(&body_bytes)?;
    assert_eq!(updated_user["name"], "Updated User");
    assert_eq!(updated_user["email"], "updated@example.com");

    // Verify DB update
    let db_name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(new_user_id.to_string())
        .fetch_one(&pool)
        .await?;
    assert_eq!(db_name, "Updated User");

    // 7. Test DELETE User
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/users/{}", new_user_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify Soft Delete (deleted_at is NOT NULL)
    let deleted_at: Option<String> = sqlx::query_scalar("SELECT deleted_at FROM users WHERE id = ?")
        .bind(new_user_id.to_string())
        .fetch_one(&pool)
        .await?;
    assert!(deleted_at.is_some(), "deleted_at should be set");

    // 8. Test 404 on deleted user update (optional, but good practice)
    // The handler requires checking existing user. If fetch_user_by_id filters out deleted users, it should return 404.
    // fetch_user_by_id clause: "WHERE id = ? AND deleted_at IS NULL" -> verified in code viewing previously.

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/users/{}", new_user_id))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"name": "Should Fail"}).to_string()))?;

    let resp: Response = app.clone().oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    Ok(())
}
