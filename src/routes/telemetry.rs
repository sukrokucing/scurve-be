use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sqlx::Acquire;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
#[allow(unused_imports)]
use crate::models::telemetry::{TelemetryBatchRequest, TelemetryIngestResponse};
use crate::utils::utc_now;

#[utoipa::path(
    post,
    path = "/telemetry/events",
    tag = "Telemetry",
    request_body = TelemetryBatchRequest,
    responses(
        (status = 202, description = "Accepted telemetry events", body = TelemetryIngestResponse),
        (status = 400, description = "Invalid telemetry payload")
    ),
    security(("bearerAuth" = []))
)]
pub async fn ingest_events(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<TelemetryBatchRequest>,
) -> AppResult<(StatusCode, Json<TelemetryIngestResponse>)> {
    if payload.events.is_empty() || payload.events.len() > 20 {
        return Err(AppError::bad_request("telemetry batch must have 1–20 events"));
    }

    for event in &payload.events {
        if event.event_id.trim().is_empty() {
            return Err(AppError::bad_request("event_id must not be blank"));
        }

        if let Some(user_id) = event.user_id {
            if user_id != auth.user_id {
                return Err(AppError::bad_request("user_id mismatch"));
            }
        }

        if event.duration_ms.unwrap_or(0) < 0 || event.intent_to_complete_ms.unwrap_or(0) < 0 {
            return Err(AppError::bad_request(
                "duration_ms and intent_to_complete_ms must be non-negative",
            ));
        }

        if let Some(metadata) = &event.metadata {
            if !metadata.is_object() {
                return Err(AppError::bad_request("metadata must be a JSON object"));
            }
        }
    }

    let mut tx = state.pool.begin().await?;
    let mut accepted = 0usize;

    for event in payload.events {
        let ingest_user_id = event.user_id.unwrap_or(auth.user_id).to_string();
        let project_id = event.project_id.map(|v| v.to_string());
        let metadata = event.metadata.map(|v| v.to_string());
        let now = utc_now();

        let result = sqlx::query(
            "INSERT OR IGNORE INTO telemetry_events \
             (event_id, event_name, occurred_at, session_id, route, user_id, project_id, view, outcome, reason, duration_ms, intent_to_complete_ms, metadata, ingested_by, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(event.event_id)
        .bind(event.event_name.as_str())
        .bind(event.occurred_at.to_rfc3339())
        .bind(event.session_id)
        .bind(event.route)
        .bind(ingest_user_id)
        .bind(project_id)
        .bind(event.view)
        .bind(event.outcome)
        .bind(event.reason)
        .bind(event.duration_ms)
        .bind(event.intent_to_complete_ms)
        .bind(metadata)
        .bind(auth.user_id.to_string())
        .bind(now)
        .execute(tx.acquire().await?)
        .await?;

        accepted += result.rows_affected() as usize;
    }

    tx.commit().await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(TelemetryIngestResponse { accepted }),
    ))
}
