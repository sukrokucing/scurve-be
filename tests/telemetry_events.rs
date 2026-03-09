use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::create_app;

mod support;

async fn setup() -> Result<(
    axum::Router,
    sqlx::SqlitePool,
    String,
    Uuid,
    support::db::TestDb,
)> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    sqlx::migrate!().run(&pool).await?;

    std::env::set_var("JWT_SECRET", "telemetry-test-secret");
    let app = create_app(pool.clone()).await?;

    let register = json!({
        "name": "Telemetry User",
        "email": "telemetry@example.com",
        "password": "password123"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(register.to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let payload: Value = serde_json::from_slice(&body)?;
    let token = payload
        .get("token")
        .and_then(Value::as_str)
        .context("missing token")?
        .to_string();
    let user_id = payload
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(Value::as_str)
        .context("missing user id")?
        .parse::<Uuid>()?;

    // Ensure strict-mode test ordering won't block this endpoint.
    sqlx::query(
        "INSERT OR IGNORE INTO user_roles (user_id, role_id, created_at) VALUES (?, '00000000-0000-0000-0000-000000000003', ?)",
    )
    .bind(user_id.to_string())
    .bind(Utc::now())
    .execute(&pool)
    .await?;

    Ok((app, pool, token, user_id, test_db))
}

#[tokio::test]
async fn telemetry_ingest_accepts_batch_and_is_idempotent() -> Result<()> {
    let (app, pool, token, user_id, _test_db) = setup().await?;
    let project_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();

    let first_id = format!("time-to-task-start-{}", session_id);
    let second_id = format!("time-to-task-completed-{}", session_id);

    let payload = json!({
        "events": [
            {
                "event_id": first_id,
                "event_name": "time_to_task.session_started",
                "occurred_at": "2026-03-04T05:38:12.232Z",
                "session_id": session_id,
                "route": "/tasks",
                "user_id": user_id,
                "project_id": project_id,
                "view": "list",
                "outcome": "completed",
                "reason": "leave-tasks-page",
                "duration_ms": 9234,
                "intent_to_complete_ms": 4033,
                "metadata": { "source": "tasks-page" }
            },
            {
                "event_id": second_id,
                "event_name": "time_to_task.completed",
                "occurred_at": "2026-03-04T05:38:21.466Z",
                "session_id": session_id,
                "route": "/tasks",
                "user_id": user_id,
                "project_id": project_id,
                "duration_ms": 9234
            },
            {
                "event_id": first_id,
                "event_name": "time_to_task.session_started",
                "occurred_at": "2026-03-04T05:38:12.232Z",
                "session_id": session_id,
                "route": "/tasks"
            }
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telemetry/events")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let parsed: Value = serde_json::from_slice(&body)?;
    assert_eq!(parsed["accepted"], 2);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM telemetry_events")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 2);

    let stored_user: String =
        sqlx::query_scalar("SELECT user_id FROM telemetry_events WHERE event_id = ?")
            .bind(second_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_user, user_id.to_string());

    Ok(())
}

#[tokio::test]
async fn telemetry_ingest_rejects_invalid_payloads() -> Result<()> {
    let (app, _pool, token, user_id, _test_db) = setup().await?;

    let bad_batches = [
        json!({ "events": [] }),
        json!({
            "events": [{
                "event_id": "x",
                "event_name": "time_to_task.completed",
                "occurred_at": "2026-03-04T05:38:12.232Z",
                "duration_ms": -1
            }]
        }),
        json!({
            "events": [{
                "event_id": "x2",
                "event_name": "time_to_task.completed",
                "occurred_at": "2026-03-04T05:38:12.232Z",
                "user_id": Uuid::new_v4(),
                "metadata": {"source":"tasks-page"}
            }]
        }),
    ];

    for batch in bad_batches {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/telemetry/events")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(batch.to_string()))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(text.contains("invalid telemetry payload"));
    }

    // Valid payload still works after invalid attempts.
    let ok_payload = json!({
        "events": [{
            "event_id": "ok-event",
            "event_name": "time_to_task.intent_marked",
            "occurred_at": "2026-03-04T05:38:12.232Z",
            "user_id": user_id
        }]
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/telemetry/events")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(ok_payload.to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    Ok(())
}
