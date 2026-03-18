#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{app, jwt};

mod support;

async fn setup() -> Result<(
    axum::Router,
    sqlx::SqlitePool,
    String,
    Uuid,
    Uuid,
    support::db::TestDb,
)> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    sqlx::migrate!().run(&pool).await?;

    std::env::set_var("JWT_SECRET", "task-health-secret");
    let app = app::create_app(pool.clone()).await?;

    let owner_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'Owner', 'owner-health@example.com', 'hash', 'local', ?, ?)",
    )
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, theme_color, created_at, updated_at) VALUES (?, ?, 'Health Project', '#123456', ?, ?)",
    )
    .bind(project_id.to_string())
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await?;

    let jwt_config = jwt::JwtConfig {
        secret: std::sync::Arc::new(b"task-health-secret".to_vec()),
        exp_hours: 1,
    };
    let token = jwt_config.encode(owner_id)?;

    Ok((app, pool, token, owner_id, project_id, test_db))
}

async fn json_response(response: axum::response::Response) -> Result<Value> {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[tokio::test]
async fn weighted_components_drive_task_health_filters_and_dashboard_rollups() -> Result<()> {
    let (app, _pool, token, _owner_id, project_id, _test_db) = setup().await?;
    let now = Utc::now();
    let baseline_start = (now - Duration::days(20)).to_rfc3339();
    let baseline_end = (now + Duration::days(20)).to_rfc3339();

    let weighted_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Weighted Task",
                        "status": "in_progress",
                        "progress_method": "weighted_components",
                        "baseline_start_at": baseline_start,
                        "baseline_end_at": baseline_end,
                        "task_weight": 2.0,
                        "due_date": (now + Duration::days(5)).to_rfc3339()
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(weighted_response.status(), StatusCode::CREATED);
    let weighted_task = json_response(weighted_response).await?;
    let weighted_task_id = weighted_task["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/projects/{}/tasks/{}/progress-components",
                    project_id, weighted_task_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "components": [
                            {
                                "name": "Analysis",
                                "component_type": "milestone",
                                "weight": 40.0,
                                "completion_pct": 100.0,
                                "sort_order": 1,
                                "completed_at": (now - Duration::days(2)).to_rfc3339()
                            },
                            {
                                "name": "Implementation",
                                "component_type": "milestone",
                                "weight": 60.0,
                                "completion_pct": 50.0,
                                "sort_order": 2
                            }
                        ]
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "replace progress components failed: {} - {}",
        status,
        String::from_utf8_lossy(&body)
    );

    let manual_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Manual Task",
                        "status": "in_progress",
                        "progress": 10,
                        "baseline_start_at": baseline_start,
                        "baseline_end_at": baseline_end,
                        "task_weight": 1.0,
                        "due_date": (now + Duration::days(5)).to_rfc3339()
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(manual_response.status(), StatusCode::CREATED);
    let manual_task = json_response(manual_response).await?;
    let manual_task_id = manual_task["id"].as_str().unwrap();

    let weighted_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}",
                    project_id, weighted_task_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(weighted_detail.status(), StatusCode::OK);
    let weighted_detail = json_response(weighted_detail).await?;
    assert_eq!(weighted_detail["progress_method"], "weighted_components");
    assert_eq!(weighted_detail["actual_progress_pct"], 70.0);
    assert_eq!(weighted_detail["progress"], 70);
    assert_eq!(
        weighted_detail["actual_progress_source"],
        "task_progress_components.weighted_completion"
    );
    assert_eq!(weighted_detail["health_status"], "ahead");
    assert_eq!(weighted_detail["execution_status"], "in_progress");
    let expected = weighted_detail["expected_progress_pct"].as_f64().unwrap();
    assert!(
        expected > 45.0 && expected < 55.0,
        "expected_progress_pct={expected}"
    );

    let manual_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks/{}", project_id, manual_task_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(manual_detail.status(), StatusCode::OK);
    let manual_detail = json_response(manual_detail).await?;
    assert_eq!(manual_detail["actual_progress_pct"], 10.0);
    assert_eq!(manual_detail["health_status"], "critical");

    let weighted_progress_write = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/projects/{}/tasks/{}/progress",
                    project_id, weighted_task_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "progress": 80, "note": "nope" }).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(weighted_progress_write.status(), StatusCode::BAD_REQUEST);

    let ahead_tasks = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?health_status=ahead&sort_by=health_status&sort_dir=asc",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(ahead_tasks.status(), StatusCode::OK);
    let ahead_tasks_json = json_response(ahead_tasks).await?;
    let ahead_tasks = ahead_tasks_json.as_array().unwrap();
    assert_eq!(ahead_tasks.len(), 1);
    assert_eq!(ahead_tasks[0]["id"], weighted_task_id);

    let sorted_tasks = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks?sort_by=health_status&sort_dir=asc",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(sorted_tasks.status(), StatusCode::OK);
    let sorted_tasks = json_response(sorted_tasks).await?;
    let sorted_tasks = sorted_tasks.as_array().unwrap();
    assert_eq!(sorted_tasks[0]["health_status"], "critical");
    assert_eq!(sorted_tasks[1]["health_status"], "ahead");

    let dashboard = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/dashboard?metric=progress",
                    project_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(dashboard.status(), StatusCode::OK);
    let dashboard = json_response(dashboard).await?;
    assert_eq!(
        dashboard["planned_source"],
        "tasks.expected_progress_pct (weighted rollup)"
    );
    assert_eq!(
        dashboard["actual_source"],
        "tasks.actual_progress_pct (weighted rollup)"
    );
    assert_eq!(dashboard["overall_progress_pct"], 50.0);
    assert!(dashboard["metric_plan"].as_array().unwrap().len() >= 1);
    assert!(dashboard["metric_actual"].as_array().unwrap().len() >= 1);

    let health_before = app
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
    assert_eq!(health_before.status(), StatusCode::OK);
    let health_before = json_response(health_before).await?;
    assert_eq!(
        health_before["planned_source"],
        "tasks.expected_progress_pct (weighted rollup)"
    );
    assert_eq!(
        health_before["actual_source"],
        "tasks.actual_progress_pct (weighted rollup)"
    );

    let update_rules = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/task-health/rules", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "rules": [
                            { "health_status": "critical", "variance_to": -25.0 },
                            { "health_status": "at_risk", "variance_from": -25.0, "variance_to": -10.0 },
                            { "health_status": "on_track", "variance_from": -10.0, "variance_to": 30.0 },
                            { "health_status": "ahead", "variance_from": 30.0 }
                        ]
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(update_rules.status(), StatusCode::OK);
    let updated_rules = json_response(update_rules).await?;
    assert_eq!(updated_rules["scope"], "project");
    assert_eq!(updated_rules["rules"].as_array().unwrap().len(), 4);

    let weighted_after_override = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/projects/{}/tasks/{}",
                    project_id, weighted_task_id
                ))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(weighted_after_override.status(), StatusCode::OK);
    let weighted_after_override = json_response(weighted_after_override).await?;
    assert_eq!(weighted_after_override["health_status"], "on_track");

    Ok(())
}

#[tokio::test]
async fn baseline_window_is_derived_from_timeline_dates_when_omitted() -> Result<()> {
    let (app, _pool, token, _owner_id, project_id, _test_db) = setup().await?;
    let now = Utc::now();
    let start = (now - Duration::days(7)).to_rfc3339();
    let end = (now + Duration::days(7)).to_rfc3339();

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Timeline-backed Task",
                        "status": "in_progress",
                        "progress": 45,
                        "start_date": start,
                        "end_date": end
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = json_response(created).await?;
    let task_id = created["id"].as_str().unwrap();
    assert_eq!(created["baseline_start_at"], created["start_date"]);
    assert_eq!(created["baseline_end_at"], created["end_date"]);
    assert!(created["expected_progress_pct"].is_number());
    assert!(created["variance_pct"].is_number());
    assert_ne!(created["health_status"], "needs_plan");

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Due-date fallback Task",
                        "status": "in_progress",
                        "progress": 20
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(updated.status(), StatusCode::CREATED);
    let updated = json_response(updated).await?;
    let update_task_id = updated["id"].as_str().unwrap();

    let start = (now - Duration::days(3)).to_rfc3339();
    let due = (now + Duration::days(5)).to_rfc3339();
    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/projects/{}/tasks/{}", project_id, update_task_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "start_date": start,
                        "due_date": due
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = json_response(updated).await?;
    assert_eq!(updated["id"], update_task_id);
    assert_eq!(updated["baseline_start_at"], updated["start_date"]);
    assert_eq!(updated["baseline_end_at"], updated["due_date"]);
    assert!(updated["expected_progress_pct"].is_number());
    assert!(updated["variance_pct"].is_number());
    assert_ne!(updated["health_status"], "needs_plan");

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{}/tasks/{}", project_id, task_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched = json_response(fetched).await?;
    assert_eq!(fetched["baseline_start_at"], fetched["start_date"]);
    assert_eq!(fetched["baseline_end_at"], fetched["end_date"]);

    Ok(())
}
