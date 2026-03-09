use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::app::AppState;
use crate::db::row_parsers;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::user::{AuthResponse, DbUser, LoginRequest, RegisterRequest, User};
use crate::utils::{hash_password, utc_now, verify_password};

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    message: String,
}

#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "Auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered", body = AuthResponse),
        (status = 409, description = "Email already in use")
    )
)]
pub async fn register(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    ensure_email_available(&state.pool, &payload.email).await?;

    let password_hash = hash_password(&payload.password)?;
    let now = utc_now();
    let user_id = uuid::Uuid::new_v4();

    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, provider_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id.to_string())
    .bind(&payload.name)
    .bind(&payload.email)
    .bind(password_hash)
    .bind("local")
    .bind(Option::<String>::None)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let db_user = fetch_user_by_id(&state.pool, user_id).await?;
    let user: User = db_user.try_into()?;
    let token = state.jwt.encode(user.id)?;

    // Log activity with request context
    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "registered",
        Some(user.id),
        &user,
        None,
        Some(ctx),
    );

    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    // Try typed mapping first
    let simple = sqlx::query_as::<_, DbUser>(
        "SELECT id, name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at FROM users WHERE email = ? AND deleted_at IS NULL",
    )
    .bind(&payload.email)
    .fetch_optional(&state.pool)
    .await;

    let db_user = match simple {
        Ok(Some(u)) => u,
        Ok(None) => return Err(AppError::unauthorized("invalid credentials")),
        Err(_) => {
            // Fallback: select textified id and parse manually
            let fallback = sqlx::query(
                "SELECT \
                   CASE WHEN typeof(id)='blob' THEN lower(substr(hex(id),1,8) || '-' || substr(hex(id),9,4) || '-' || substr(hex(id),13,4) || '-' || substr(hex(id),17,4) || '-' || substr(hex(id),21)) ELSE id END as id, \
                   name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at \
                 FROM users WHERE email = ? AND deleted_at IS NULL",
            )
            .bind(&payload.email)
            .fetch_optional(&state.pool)
            .await?;

            let row = fallback.ok_or_else(|| AppError::unauthorized("invalid credentials"))?;
            row_parsers::db_user_from_row(&row)?
        }
    };

    let password_ok = verify_password(&payload.password, &db_user.password_hash)?;
    if !password_ok {
        return Err(AppError::unauthorized("invalid credentials"));
    }

    let token = state.jwt.encode(db_user.id)?;
    let user: User = db_user.try_into()?;

    Ok(Json(AuthResponse { token, user }))
}

#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "Auth",
    responses((status = 200, description = "Current user", body = User)),
    security(("bearerAuth" = []))
)]
pub async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<User>> {
    let db_user = fetch_user_by_id(&state.pool, auth.user_id).await?;
    let user: User = db_user.try_into()?;
    Ok(Json(user))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "Auth",
    responses((status = 200, description = "Logout acknowledged")),
    security(("bearerAuth" = []))
)]
pub async fn logout(_auth: AuthUser) -> AppResult<Json<MessageResponse>> {
    Ok(Json(MessageResponse {
        message: "Logged out".to_string(),
    }))
}

// --- Password Reset Types ---

use utoipa::ToSchema;

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    #[schema(example = "user@example.com")]
    pub email: String,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    #[schema(example = "abc123token")]
    pub token: String,
    #[schema(example = "NewSecureP@ss456")]
    pub new_password: String,
}

// --- Forgot Password ---

/// Request password reset
///
/// Generates a password reset token and returns it. In production, this should send an email.
#[utoipa::path(
    post,
    path = "/auth/forgot-password",
    tag = "Auth",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Reset token generated (for dev; in prod would send email)", body = MessageResponse),
        (status = 404, description = "User not found")
    )
)]
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> impl axum::response::IntoResponse {
    use crate::utils::utc_now;
    use rand::Rng;
    use sha2::{Digest, Sha256};

    // Find user by email
    let user_row = sqlx::query("SELECT id FROM users WHERE email = ? AND deleted_at IS NULL")
        .bind(&payload.email)
        .fetch_optional(&state.pool)
        .await?;

    let user_row = match user_row {
        Some(row) => row,
        None => return Err(AppError::not_found("User not found")),
    };

    let user_id: String = sqlx::Row::get(&user_row, "id");

    // Generate a random token
    let raw_token: String = {
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect()
    };

    // Hash the token for storage
    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    let now = utc_now();
    let expires_at = now + chrono::Duration::hours(1);
    let token_id = uuid::Uuid::new_v4();

    sqlx::query(
        "INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(token_id.to_string())
    .bind(&user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .bind(now)
    .execute(&state.pool)
    .await?;

    // In production, send email with raw_token. For dev, return it directly.
    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: format!("Reset token (dev only): {}", raw_token),
        }),
    ))
}

// --- Reset Password ---

/// Reset password with token
///
/// Validates the reset token and sets a new password.
#[utoipa::path(
    post,
    path = "/auth/reset-password",
    tag = "Auth",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset successful", body = MessageResponse),
        (status = 400, description = "Invalid or expired token")
    )
)]
pub async fn reset_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> impl axum::response::IntoResponse {
    use crate::utils::{hash_password, utc_now};
    use sha2::{Digest, Sha256};

    // Hash the provided token
    let mut hasher = Sha256::new();
    hasher.update(payload.token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    let now = utc_now();

    // Find valid token
    let token_row = sqlx::query(
        "SELECT id, user_id FROM password_reset_tokens WHERE token_hash = ? AND expires_at > ? AND used_at IS NULL"
    )
    .bind(&token_hash)
    .bind(now)
    .fetch_optional(&state.pool)
    .await?;

    let token_row = match token_row {
        Some(row) => row,
        None => return Err(AppError::bad_request("Invalid or expired token")),
    };

    let token_id: String = sqlx::Row::get(&token_row, "id");
    let user_id: String = sqlx::Row::get(&token_row, "user_id");

    // Mark token as used
    sqlx::query("UPDATE password_reset_tokens SET used_at = ? WHERE id = ?")
        .bind(now)
        .bind(&token_id)
        .execute(&state.pool)
        .await?;

    // Update user password
    let password_hash = hash_password(&payload.new_password)?;
    sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
        .bind(password_hash)
        .bind(now)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;

    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: "Password reset successful".to_string(),
        }),
    ))
}

async fn ensure_email_available(pool: &SqlitePool, email: &str) -> AppResult<()> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(1) FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(email)
            .fetch_one(pool)
            .await?;

    if count > 0 {
        return Err(AppError::conflict("email already in use"));
    }

    Ok(())
}

async fn fetch_user_by_id(pool: &SqlitePool, user_id: uuid::Uuid) -> AppResult<DbUser> {
    let simple = sqlx::query_as::<_, DbUser>(
        "SELECT id, name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at FROM users WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some(u) = simple {
        return Ok(u);
    }

    // Fallback: handle blob/text mixed UUID storage by selecting textified id
    let id_case = crate::db::uuid_sql::case_uuid("id");
    let match_id = crate::db::uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "SELECT {} , name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at FROM users WHERE {} AND deleted_at IS NULL",
        id_case, match_id
    );

    let fallback = sqlx::query(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;

    if let Some(row) = fallback {
        return Ok(row_parsers::db_user_from_row(&row)?);
    }

    Err(AppError::not_found("user not found"))
}
