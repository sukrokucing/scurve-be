use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use tracing::error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("token error: {0}")]
    Token(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("internal server error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    pub fn token(err: impl Into<String>) -> Self {
        Self::Token(err.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log all AppErrors so they are visible in server logs
        error!(%self, "request failed");

        let status = match self {
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Configuration(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Token(_) => StatusCode::UNAUTHORIZED,
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let message = self.to_string();
        let error = match &self {
            AppError::Unauthorized(_) => "unauthorized",
            AppError::Forbidden(_) => "forbidden",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::BadRequest(_) => "bad_request",
            AppError::Configuration(_) => "configuration",
            AppError::Token(_) => "token",
            AppError::Database(_) => "database",
            AppError::Internal(_) => "internal",
        };

        // Optionally include debug detail in the JSON response when
        // SHOW_ERRORS env var is set to `1` or `true`. This is intended
        // for local debugging only; avoid enabling in production.
        let show = std::env::var("SHOW_ERRORS").map(|v| {
            let v = v.to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes"
        }).unwrap_or(false);

        let detail = if show {
            Some(format!("{:?}", self))
        } else {
            None
        };

        let payload = ErrorResponse {
            error: error.to_string(),
            message,
            detail,
        };

        (status, Json(payload)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value.to_string())
    }
}
