use axum::extract::State;
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::errors::AppResult;
use sqlx::query_scalar;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub db_ok: bool,
    pub db_error: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "Health",
    responses((status = 200, description = "Health check", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    // Lightweight DB check
    let db_check = query_scalar::<_, i64>("SELECT 1").fetch_one(&state.pool).await;

    match db_check {
        Ok(_) => Ok(Json(HealthResponse { status: "ok", db_ok: true, db_error: None })),
        Err(e) => Ok(Json(HealthResponse { status: "ok", db_ok: false, db_error: Some(e.to_string()) })),
    }
}
