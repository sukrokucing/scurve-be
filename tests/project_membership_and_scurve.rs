#![allow(clippy::uninlined_format_args)]

use std::sync::Arc;

use axum::{
    body::{self, Body},
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{app, jwt};

mod support;

async fn setup() -> anyhow::Result<(Router, SqlitePool, jwt::JwtConfig, support::db::TestDb)> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Ensure new migrations are applied even when the template DB lags behind.
    sqlx::migrate!().run(&pool).await?;

    std::env::set_var("JWT_SECRET", "project-membership-scurve-tests");
    let app = app::create_app(pool.clone()).await?;
    let jwt_config = jwt::JwtConfig {
        secret: Arc::new(b"project-membership-scurve-tests".to_vec()),
        exp_hours: 1,
    };

    Ok((app, pool, jwt_config, test_db))
}

async fn seed_user(pool: &SqlitePool, name: &str, email: &str) -> anyhow::Result<Uuid> {
    let user_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at)
         VALUES (?, ?, ?, 'hash', 'local', ?, ?)",
    )
    .bind(user_id.to_string())
    .bind(name)
    .bind(email)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(user_id)
}

async fn create_project_via_api(app: &Router, token: &str, name: &str) -> anyhow::Result<Uuid> {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "name": name, "description": "desc", "theme_color": "#123456" })
                        .to_string(),
                ))?,
        )
        .await?;
    let status = res.status();
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "project creation failed: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body)?;
    let project_id = payload
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing project id"))?;
    Ok(Uuid::parse_str(project_id)?)
}

#[tokio::test]
async fn project_membership_delete_revokes_access() -> anyhow::Result<()> {
    let (app, pool, jwt_config, _test_db) = setup().await?;

    let owner_id = seed_user(&pool, "Owner", "owner-membership@example.com").await?;
    let member_id = seed_user(&pool, "Member", "member-membership@example.com").await?;
    let owner_token = jwt_config.encode(owner_id)?;
    let member_token = jwt_config.encode(member_id)?;

    let project_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at)
         VALUES (?, ?, 'Membership Project', '#112233', ?, ?)",
    )
    .bind(project_id.to_string())
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, progress, created_at, updated_at)
         VALUES (?, ?, 'Membership Task', 'pending', 0, ?, ?)",
    )
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let viewer_role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE name = 'viewer'")
        .fetch_one(&pool)
        .await?;
    let unclassified_resource_role_id: String =
        sqlx::query_scalar("SELECT id FROM resource_roles WHERE name = 'unclassified'")
            .fetch_one(&pool)
            .await?;
    let membership_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO project_members (id, project_id, user_id, access_role_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(membership_id.to_string())
    .bind(project_id.to_string())
    .bind(member_id.to_string())
    .bind(viewer_role_id)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO project_member_resource_roles (id, membership_id, resource_role_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(membership_id.to_string())
    .bind(unclassified_resource_role_id)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", member_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{}/members/{}", project_id, member_id))
                .header("Authorization", format!("Bearer {}", owner_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", member_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn task_description_is_auto_filled_and_cannot_be_emptied() -> anyhow::Result<()> {
    let (app, pool, jwt_config, _test_db) = setup().await?;

    let user_id = seed_user(&pool, "Desc User", "desc-user@example.com").await?;
    let token = jwt_config.encode(user_id)?;
    let project_id = create_project_via_api(&app, &token, "Description Project").await?;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "title": "Quick Task" }).to_string()))?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let created: Value = serde_json::from_slice(&body)?;
    let task_id = Uuid::parse_str(created["id"].as_str().unwrap())?;
    assert_eq!(created["description"], "[Quick Add] Quick Task");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "description": "   " }).to_string()))?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "description": "Detailed description" }).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let updated: Value = serde_json::from_slice(&body)?;
    assert_eq!(updated["description"], "Detailed description");

    Ok(())
}

#[tokio::test]
async fn s_curve_health_and_portfolio_summary_return_expected_shape() -> anyhow::Result<()> {
    let (app, pool, jwt_config, _test_db) = setup().await?;

    let user_id = seed_user(&pool, "Health User", "health-user@example.com").await?;
    let token = jwt_config.encode(user_id)?;
    let project_id = create_project_via_api(&app, &token, "Health Project").await?;

    let now = Utc::now();
    sqlx::query(
        "INSERT INTO project_plan (id, project_id, date, planned_progress, planned_hours, planned_cost, currency, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind((now - Duration::days(7)).to_rfc3339())
    .bind(30_i32)
    .bind(40.0_f64)
    .bind(4_000.0_f64)
    .bind("USD")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO project_plan (id, project_id, date, planned_progress, planned_hours, planned_cost, currency, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind((now + Duration::days(7)).to_rfc3339())
    .bind(80_i32)
    .bind(100.0_f64)
    .bind(10_000.0_f64)
    .bind("USD")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, progress, created_at, updated_at)
         VALUES (?, ?, 'Health Task', 'doing', 55, ?, ?)",
    )
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;
    let unclassified_resource_role_id: String =
        sqlx::query_scalar("SELECT id FROM resource_roles WHERE name = 'unclassified'")
            .fetch_one(&pool)
            .await?;
    sqlx::query(
        "INSERT INTO work_logs (
            id, project_id, task_id, user_id, resource_role_id, hours,
            hourly_rate_snapshot, currency_snapshot, cost_amount, work_date, note, source, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'manual', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(task_id.to_string())
    .bind(user_id.to_string())
    .bind(unclassified_resource_role_id)
    .bind(45.0_f64)
    .bind(100.0_f64)
    .bind("USD")
    .bind(4_500.0_f64)
    .bind(now.date_naive().to_string())
    .bind("metric seed")
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let res: Response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/s-curve/health?metric=progress",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let progress_health: Value = serde_json::from_slice(&body)?;
    assert_eq!(progress_health["metric"], "progress");
    assert!(
        progress_health["elapsed_time_pct"].is_number()
            || progress_health["elapsed_time_pct"].is_null()
    );
    assert!(progress_health.get("rule_50_70_status").is_some());

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/s-curve/health?metric=hours",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let hours_health: Value = serde_json::from_slice(&body)?;
    assert_eq!(hours_health["metric"], "hours");
    assert_eq!(hours_health["metric_supported"], true);
    assert_eq!(hours_health["data_status"], "ok");
    assert!(hours_health["planned_pct"].is_number());
    assert!(hours_health["actual_pct"].is_number());
    assert!(hours_health["variance_pct"].is_number());
    assert!(hours_health["stage"].is_string());
    assert!(hours_health["rule_50_70_status"].is_string());
    assert_eq!(hours_health["unit"], "hours");
    assert_eq!(
        hours_health["planned_source"],
        "project_plan.planned_hours (cumulative)"
    );
    assert_eq!(hours_health["actual_source"], "work_logs.hours (sum)");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/s-curve/health?metric=cost",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let cost_health: Value = serde_json::from_slice(&body)?;
    assert_eq!(cost_health["metric"], "cost");
    assert_eq!(cost_health["metric_supported"], true);
    assert_eq!(cost_health["data_status"], "ok");
    assert!(cost_health["planned_pct"].is_number());
    assert!(cost_health["actual_pct"].is_number());
    assert_eq!(cost_health["currency"], "USD");
    assert_eq!(cost_health["unit"], "currency");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/portfolio/s-curve/summary?metric=hours")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let portfolio: Value = serde_json::from_slice(&body)?;
    assert_eq!(portfolio["metric"], "hours");
    assert_eq!(portfolio["metric_supported"], true);
    assert_eq!(portfolio["data_status"], "ok");
    assert_eq!(portfolio["project_count"], 1);
    assert_eq!(portfolio["projects"].as_array().unwrap().len(), 1);
    assert_eq!(portfolio["projects"][0]["data_status"], "ok");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/portfolio/s-curve/summary?metric=cost")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let portfolio_cost: Value = serde_json::from_slice(&body)?;
    assert_eq!(portfolio_cost["metric"], "cost");
    assert_eq!(portfolio_cost["metric_supported"], true);
    assert_eq!(portfolio_cost["data_status"], "ok");
    assert_eq!(portfolio_cost["currency"], "USD");

    Ok(())
}

#[tokio::test]
async fn users_me_projects_returns_project_scope_rows() -> anyhow::Result<()> {
    let (app, pool, jwt_config, _test_db) = setup().await?;

    let user_id = seed_user(&pool, "Scope User", "scope-user@example.com").await?;
    let token = jwt_config.encode(user_id)?;
    let project_id = create_project_via_api(&app, &token, "Scope Project").await?;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/users/me/projects")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(res.status(), StatusCode::OK);

    let body = body::to_bytes(res.into_body(), usize::MAX).await?;
    let payload: Value = serde_json::from_slice(&body)?;
    let items = payload
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected array response"))?;
    assert!(
        items
            .iter()
            .any(|item| item.get("project_id").and_then(Value::as_str)
                == Some(&project_id.to_string())),
        "expected project_id {} in /users/me/projects response: {}",
        project_id,
        payload
    );

    Ok(())
}
