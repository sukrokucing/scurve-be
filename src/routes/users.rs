use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;

use crate::app::AppState;
use crate::db::{row_parsers, uuid_sql};
use crate::errors::AppError;
use crate::jwt::AuthUser;
use crate::models::user::User;

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// List users (admin)
///
/// Returns a paginated list of users, optionally filtered by name or email.
/// Accessible only to admins or users with `user.view` permission.
#[utoipa::path(
    get,
    path = "/users",
    tag = "Users",
    params(
        ("q" = Option<String>, Query, description = "Optional search query (name or email)"),
        ("page" = Option<u32>, Query, description = "Page number (1-based, default 1)"),
        ("per_page" = Option<u32>, Query, description = "Items per page (default 25, max 100)"),
    ),
    responses(
        (status = 200, description = "List users", body = Vec<User>, headers(
            ("X-Total-Count" = i64, description = "Total number of users matching the query")
        )),
    ),
    security(("bearerAuth" = []))
)]
pub async fn list_users(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(params): Query<ListUsersQuery>,
) -> Result<(HeaderMap, Json<Vec<User>>), AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(25).min(100);
    let offset = (page - 1) * per_page;

    let id_case = uuid_sql::case_uuid("id");

    let (rows, total_count) = if let Some(q) = params.q {
        let search_pattern = format!("%{}%", q);

        let sql = format!(
            "SELECT {}, name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at
             FROM users
             WHERE (name LIKE ? OR email LIKE ?) AND deleted_at IS NULL
             ORDER BY name
             LIMIT ? OFFSET ?",
            id_case
        );

        let rows = sqlx::query(&sql)
            .bind(&search_pattern)
            .bind(&search_pattern)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE (name LIKE ? OR email LIKE ?) AND deleted_at IS NULL",
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .fetch_one(&state.pool)
        .await?;

        (rows, count)
    } else {
        let sql = format!(
            "SELECT {}, name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at
             FROM users
             WHERE deleted_at IS NULL
             ORDER BY name
             LIMIT ? OFFSET ?",
            id_case
        );

        let rows = sqlx::query(&sql)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
            .fetch_one(&state.pool)
            .await?;

        (rows, count)
    };

    let mut users = Vec::with_capacity(rows.len());
    for row in rows {
        let db_user = row_parsers::db_user_from_row(&row)?;
        users.push(User::try_from(db_user)?);
    }

    let mut headers = HeaderMap::new();
    headers.insert("X-Total-Count", total_count.into());

    Ok((headers, Json(users)))
}

// --- Request/Response Types ---

use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    #[schema(example = "New Admin")]
    pub name: String,
    #[schema(example = "newadmin@example.com")]
    pub email: String,
    #[schema(example = "SecureP@ss123")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    #[schema(example = "Updated Name")]
    pub name: Option<String>,
    #[schema(example = "updated@example.com")]
    pub email: Option<String>,
    #[schema(example = "NewP@ssword456")]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeletedResponse {
    pub message: String,
}

// --- Create User ---

/// Create a new user (admin)
///
/// Creates a new user with password hash. Requires `user.manage` permission.
#[utoipa::path(
    post,
    path = "/users",
    tag = "Users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = User),
        (status = 409, description = "Email already in use"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn create_user(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(axum::http::StatusCode, Json<User>), AppError> {
    use crate::utils::{hash_password, utc_now};

    // Check email uniqueness
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(&payload.email)
            .fetch_one(&state.pool)
            .await?;

    if existing.unwrap_or(0) > 0 {
        return Err(AppError::conflict("Email already in use"));
    }

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

    let user = fetch_user_by_id(&state.pool, user_id).await?;

    Ok((axum::http::StatusCode::CREATED, Json(user)))
}

// --- Update User ---

/// Update a user (admin)
///
/// Updates user details. Requires `user.manage` permission.
#[utoipa::path(
    put,
    path = "/users/{id}",
    tag = "Users",
    params(("id" = String, Path, description = "User ID")),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = User),
        (status = 404, description = "User not found"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn update_user(
    State(state): State<AppState>,
    _auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<User>, AppError> {
    use crate::utils::{hash_password, utc_now};

    // Verify user exists
    let _ = fetch_user_by_id(&state.pool, id).await?;

    let now = utc_now();

    if let Some(ref name) = payload.name {
        sqlx::query("UPDATE users SET name = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(now)
            .bind(id.to_string())
            .execute(&state.pool)
            .await?;
    }

    if let Some(ref email) = payload.email {
        // Check email uniqueness (excluding current user)
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE email = ? AND id != ? AND deleted_at IS NULL",
        )
        .bind(email)
        .bind(id.to_string())
        .fetch_one(&state.pool)
        .await?;

        if existing > 0 {
            return Err(AppError::conflict("Email already in use"));
        }

        sqlx::query("UPDATE users SET email = ?, updated_at = ? WHERE id = ?")
            .bind(email)
            .bind(now)
            .bind(id.to_string())
            .execute(&state.pool)
            .await?;
    }

    if let Some(ref password) = payload.password {
        let password_hash = hash_password(password)?;
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
            .bind(password_hash)
            .bind(now)
            .bind(id.to_string())
            .execute(&state.pool)
            .await?;
    }

    let user = fetch_user_by_id(&state.pool, id).await?;
    Ok(Json(user))
}

// --- Delete User (Soft Delete) ---

/// Delete a user (admin)
///
/// Soft-deletes a user by setting deleted_at. Requires `user.manage` permission.
#[utoipa::path(
    delete,
    path = "/users/{id}",
    tag = "Users",
    params(("id" = String, Path, description = "User ID")),
    responses(
        (status = 200, description = "User deleted", body = DeletedResponse),
        (status = 404, description = "User not found"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn delete_user(
    State(state): State<AppState>,
    _auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<DeletedResponse>, AppError> {
    use crate::utils::utc_now;

    // Verify user exists
    let _ = fetch_user_by_id(&state.pool, id).await?;

    let now = utc_now();

    sqlx::query("UPDATE users SET deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .execute(&state.pool)
        .await?;

    Ok(Json(DeletedResponse {
        message: "User deleted".to_string(),
    }))
}

// --- Helper ---

async fn fetch_user_by_id(pool: &sqlx::SqlitePool, user_id: uuid::Uuid) -> Result<User, AppError> {
    use crate::models::user::DbUser;

    let simple = sqlx::query_as::<_, DbUser>(
        "SELECT id, name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at FROM users WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some(u) = simple {
        return Ok(User::try_from(u)?);
    }

    // Fallback for blob/text mixed UUID storage
    let id_case = uuid_sql::case_uuid("id");
    let match_id = uuid_sql::match_uuid_clause("id");
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
        let db_user = row_parsers::db_user_from_row(&row)?;
        return Ok(User::try_from(db_user)?);
    }

    Err(AppError::not_found("User not found"))
}
