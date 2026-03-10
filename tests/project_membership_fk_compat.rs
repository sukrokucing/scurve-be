use anyhow::{Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{fs, path::Path, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{create_app, jwt::JwtConfig};

#[tokio::test]
async fn blob_user_ids_do_not_break_project_and_member_creation_with_foreign_keys() -> Result<()> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("scurve.sqlite");
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir for test db")?;
    let db_path = tmp_dir.path().join("scurve.sqlite");
    fs::copy(&source, &db_path).with_context(|| {
        format!(
            "failed to clone template database from {} to {}",
            source.display(),
            db_path.display()
        )
    })?;

    let opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    let owner_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    let now = Utc::now();

    let owner_email = format!("blob.owner.{}@example.com", Uuid::new_v4());
    let member_email = format!("blob.member.{}@example.com", Uuid::new_v4());

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at)
         VALUES (?, 'Blob Owner', ?, 'hash', 'local', ?, ?)",
    )
    .bind(owner_id.as_bytes().to_vec())
    .bind(&owner_email)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at)
         VALUES (?, 'Blob Member', ?, 'hash', 'local', ?, ?)",
    )
    .bind(member_id.as_bytes().to_vec())
    .bind(&member_email)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    std::env::set_var("JWT_SECRET", "project-members-fk-compat-secret");
    let app = create_app(pool.clone()).await?;

    let jwt = JwtConfig {
        secret: Arc::new(b"project-members-fk-compat-secret".to_vec()),
        exp_hours: 1,
    };
    let token = jwt.encode(owner_id)?;

    let create_project_req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({
                "name": "FK Compat Project",
                "description": "Ensure project owner membership works for blob user ids"
            })
            .to_string(),
        ))?;

    let create_project_resp = app.clone().oneshot(create_project_req).await?;
    let create_project_status = create_project_resp.status();
    let create_project_body = to_bytes(create_project_resp.into_body(), usize::MAX).await?;
    assert_eq!(
        create_project_status,
        StatusCode::CREATED,
        "project creation failed: {}",
        String::from_utf8_lossy(&create_project_body)
    );

    let created_project: Value = serde_json::from_slice(&create_project_body)?;
    let project_id = created_project
        .get("id")
        .and_then(|v| v.as_str())
        .context("missing project id in response")?
        .to_string();

    let viewer_role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE name = 'viewer'")
        .fetch_one(&pool)
        .await?;

    let create_member_req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{project_id}/members"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({
                "user_id": member_id,
                "role_id": viewer_role_id
            })
            .to_string(),
        ))?;

    let create_member_resp = app.clone().oneshot(create_member_req).await?;
    let create_member_status = create_member_resp.status();
    let create_member_body = to_bytes(create_member_resp.into_body(), usize::MAX).await?;
    assert_eq!(
        create_member_status,
        StatusCode::CREATED,
        "member creation failed: {}",
        String::from_utf8_lossy(&create_member_body)
    );

    Ok(())
}
