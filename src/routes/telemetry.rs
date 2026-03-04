use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sqlx::Acquire;
use tracing::error;

use crate::app::AppState;
use crate::jwt::AuthUser;
#[allow(unused_imports)]
use crate::models::telemetry::{
    TelemetryBatchRequest, TelemetryErrorResponse, TelemetryIngestResponse,
};
use crate::utils::utc_now;

#[utoipa::path(
    post,
    path = "/telemetry/events",
    tag = "Telemetry",
    request_body = TelemetryBatchRequest,
    responses(
        (status = 202, description = "Accepted telemetry events", body = TelemetryIngestResponse),
        (status = 400, description = "Invalid telemetry payload", body = TelemetryErrorResponse)
    ),
    security(("bearerAuth" = []))
)]
pub async fn ingest_events(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<TelemetryBatchRequest>,
) -> Result<(StatusCode, Json<TelemetryIngestResponse>), (StatusCode, Json<TelemetryErrorResponse>)> {
    if payload.events.is_empty() || payload.events.len() > 20 {
        return Err(invalid_payload());
    }

    for event in &payload.events {
        if event.event_id.trim().is_empty() {
            return Err(invalid_payload());
        }

        if let Some(user_id) = event.user_id {
            if user_id != auth.user_id {
                return Err(invalid_payload());
            }
        }

        if event.duration_ms.unwrap_or(0) < 0 || event.intent_to_complete_ms.unwrap_or(0) < 0 {
            return Err(invalid_payload());
        }

        if let Some(metadata) = &event.metadata {
            if !metadata.is_object() {
                return Err(invalid_payload());
            }
        }
    }

    let mut tx = state.pool.begin().await.map_err(internal_error)?;
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
        .execute(tx.acquire().await.map_err(internal_error)?)
        .await
        .map_err(internal_error)?;

        accepted += result.rows_affected() as usize;
    }

    tx.commit().await.map_err(internal_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(TelemetryIngestResponse { accepted }),
    ))
}

fn invalid_payload() -> (StatusCode, Json<TelemetryErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(TelemetryErrorResponse {
            message: "invalid telemetry payload".to_string(),
        }),
    )
}

fn internal_error(err: sqlx::Error) -> (StatusCode, Json<TelemetryErrorResponse>) {
    error!(error = %err, "telemetry ingest failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(TelemetryErrorResponse {
            message: "internal server error".to_string(),
        }),
    )
}
