//! RBAC Admin API Routes
//!
//! Endpoints for managing roles, permissions, and user assignments.
//! All RBAC modifications are logged to the activity log with Critical severity.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, delete},
    Json, Router,
    middleware::from_fn_with_state,
};
use crate::authz;
use chrono::Utc;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;
use crate::events::{log_activity_with_context, RequestContext};
use crate::jwt::AuthUser;
use crate::models::rbac::*;

// =============================================================================
// ROUTER
// =============================================================================

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        // Roles
        .route("/roles", get(list_roles)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_view))
            .post(create_role)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_manage)))
        .route("/roles/:role_id", get(get_role)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_view))
            .delete(delete_role)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_manage)))
        .route("/roles/:role_id/permissions", get(get_role_permissions)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_view))
            .post(assign_permission_to_role)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_manage)))
        .route(
            "/roles/:role_id/permissions/:permission_id",
            delete(delete_permission_from_role)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_manage)),
        )
        // Permissions
        .route("/permissions", get(list_permissions)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_permission_view))
            .post(create_permission)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_permission_manage)))
        // User role assignments
        .route("/users/:user_id/roles", get(get_user_roles)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_view))
            .post(assign_role_to_user)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_manage)))
        .route("/users/:user_id/roles/:role_id", delete(revoke_role_from_user)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_role_manage)))
        // User direct permissions
        .route("/users/:user_id/permissions", get(get_user_permissions)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_permission_view))
            .post(grant_permission_to_user)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_permission_manage)))
        // Effective permissions (computed)
        .route("/users/:user_id/effective-permissions", get(get_effective_permissions)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_user_view)))
}

// =============================================================================
// ROLE ENDPOINTS
// =============================================================================

/// List all roles
#[utoipa::path(
    get,
    path = "/rbac/roles",
    tag = "RBAC",
    responses(
        (status = 200, description = "List of roles", body = Vec<Role>),
    ),
    security(("bearerAuth" = []))
)]
async fn list_roles(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Role>>, AppError> {
    use crate::db::uuid_sql::case_uuid;
    let id_case = case_uuid("id");
    let sql = format!("SELECT {}, name, description, created_at, updated_at FROM roles ORDER BY name", id_case);

    let rows = sqlx::query(&sql)
    .fetch_all(&state.pool)
    .await?;

    let roles: Vec<Role> = rows.iter().map(|r| Role {
        id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
        name: r.get("name"),
        description: r.get("description"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }).collect();

    Ok(Json(roles))
}

/// Create a new role
#[utoipa::path(
    post,
    path = "/rbac/roles",
    tag = "RBAC",
    request_body = RoleCreateRequest,
    responses(
        (status = 201, description = "Role created", body = Role),
        (status = 409, description = "Role name already exists"),
    ),
    security(("bearerAuth" = []))
)]
async fn create_role(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<RoleCreateRequest>,
) -> Result<(StatusCode, Json<Role>), AppError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO roles (id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(id.to_string())
    .bind(&req.name)
    .bind(&req.description)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let role = Role {
        id,
        name: req.name,
        description: req.description,
        created_at: now,
        updated_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "created",
        Some(auth.user_id),
        &role,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok((StatusCode::CREATED, Json(role)))
}

/// Get a role by ID
#[utoipa::path(
    get,
    path = "/rbac/roles/{role_id}",
    tag = "RBAC",
    params(
        ("role_id" = Uuid, Path, description = "Role ID"),
    ),
    responses(
        (status = 200, description = "Role details", body = Role),
        (status = 404, description = "Role not found"),
    ),
    security(("bearerAuth" = []))
)]
async fn get_role(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(role_id): Path<Uuid>,
) -> Result<Json<Role>, AppError> {
    use crate::db::uuid_sql::{case_uuid, match_uuid_clause};
    let id_case = case_uuid("id");
    let match_clause = match_uuid_clause("id");
    let sql = format!("SELECT {}, name, description, created_at, updated_at FROM roles WHERE {}", id_case, match_clause);

    let row = sqlx::query(&sql)
    .bind(role_id.to_string())
    .bind(role_id.to_string())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("Role not found"))?;

    let role = Role {
        id: Uuid::parse_str(row.get::<&str, _>("id")).unwrap_or_default(),
        name: row.get("name"),
        description: row.get("description"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(Json(role))
}

/// Delete a role
#[utoipa::path(
    delete,
    path = "/rbac/roles/{role_id}",
    tag = "RBAC",
    params(
        ("role_id" = Uuid, Path, description = "Role ID"),
    ),
    responses(
        (status = 204, description = "Role deleted"),
        (status = 404, description = "Role not found"),
    ),
    security(("bearerAuth" = []))
)]
async fn delete_role(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    use crate::db::uuid_sql::{case_uuid, match_uuid_clause};
    let id_case = case_uuid("id");
    let match_clause = match_uuid_clause("id");
    let fetch_sql = format!("SELECT {}, name, description, created_at, updated_at FROM roles WHERE {}", id_case, match_clause);

    let row = sqlx::query(&fetch_sql)
    .bind(role_id.to_string())
    .bind(role_id.to_string())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("Role not found"))?;

    let role = Role {
        id: Uuid::parse_str(row.get::<&str, _>("id")).unwrap_or_default(),
        name: row.get("name"),
        description: row.get("description"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    let delete_sql = format!("DELETE FROM roles WHERE {}", match_clause);
    sqlx::query(&delete_sql)
        .bind(role_id.to_string())
        .bind(role_id.to_string())
        .execute(&state.pool)
        .await?;

    log_activity_with_context(
        &state.event_bus,
        "deleted",
        Some(auth.user_id),
        &role,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Assign a permission to a role
#[utoipa::path(
    post,
    path = "/rbac/roles/{role_id}/permissions",
    tag = "RBAC",
    params(
        ("role_id" = Uuid, Path, description = "Role ID"),
    ),
    request_body = AssignPermissionToRoleRequest,
    responses(
        (status = 201, description = "Permission assigned"),
        (status = 404, description = "Role not found"),
    ),
    security(("bearerAuth" = []))
)]
async fn assign_permission_to_role(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
    Json(req): Json<AssignPermissionToRoleRequest>,
) -> Result<StatusCode, AppError> {
    use crate::db::uuid_sql::match_uuid_clause;
    let now = Utc::now();

    let role_match = match_uuid_clause("role_id");
    let perm_match = match_uuid_clause("permission_id");

    // Check if it already exists using match clauses to be safe
    let check_sql = format!("SELECT 1 FROM role_permissions WHERE {} AND {}", role_match, perm_match);
    let existing = sqlx::query(&check_sql)
        .bind(role_id.to_string())
        .bind(role_id.to_string())
        .bind(req.permission_id.to_string())
        .bind(req.permission_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if existing.is_none() {
        // Insert as strings (standard for new writes)
        sqlx::query(
            "INSERT INTO role_permissions (role_id, permission_id, created_at) VALUES (?, ?, ?)"
        )
        .bind(role_id.to_string())
        .bind(req.permission_id.to_string())
        .bind(now)
        .execute(&state.pool)
        .await?;
    }

    let assignment = RolePermission {
        role_id,
        permission_id: req.permission_id,
        created_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "assigned",
        Some(auth.user_id),
        &assignment,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok(StatusCode::CREATED)
}

/// Get permissions assigned to a role
#[utoipa::path(
    get,
    path = "/rbac/roles/{role_id}/permissions",
    tag = "RBAC",
    params(
        ("role_id" = Uuid, Path, description = "Role ID"),
    ),
    responses(
        (status = 200, description = "List of assigned permissions", body = Vec<Permission>),
    ),
    security(("bearerAuth" = []))
)]
async fn get_role_permissions(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(role_id): Path<Uuid>,
) -> Result<Json<Vec<Permission>>, AppError> {
    use crate::db::uuid_sql::{case_uuid, match_uuid_clause};
    let id_case = case_uuid("p.id");
    let role_match = match_uuid_clause("rp.role_id");

    let sql = format!(
        r#"
        SELECT {}, p.name, p.description, p.created_at, p.updated_at
        FROM permissions p
        INNER JOIN role_permissions rp ON p.id = rp.permission_id
        WHERE {}
        ORDER BY p.name
        "#,
        id_case, role_match
    );

    let rows = sqlx::query(&sql)
    .bind(role_id.to_string())
    .bind(role_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    let permissions: Vec<Permission> = rows.iter().map(|r| Permission {
        id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
        name: r.get("name"),
        description: r.get("description"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }).collect();

    Ok(Json(permissions))
}

/// Remove a permission from a role
#[utoipa::path(
    delete,
    path = "/rbac/roles/{role_id}/permissions/{permission_id}",
    tag = "RBAC",
    params(
        ("role_id" = Uuid, Path, description = "Role ID"),
        ("permission_id" = Uuid, Path, description = "Permission ID"),
    ),
    responses(
        (status = 204, description = "Permission removed from role"),
    ),
    security(("bearerAuth" = []))
)]
async fn delete_permission_from_role(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path((role_id, permission_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    use crate::db::uuid_sql::match_uuid_clause;
    let now = Utc::now();

    let role_match = match_uuid_clause("role_id");
    let perm_match = match_uuid_clause("permission_id");
    let sql = format!("DELETE FROM role_permissions WHERE {} AND {}", role_match, perm_match);

    sqlx::query(&sql)
        .bind(role_id.to_string())
        .bind(role_id.to_string())
        .bind(permission_id.to_string())
        .bind(permission_id.to_string())
        .execute(&state.pool)
        .await?;

    let assignment = RolePermission {
        role_id,
        permission_id,
        created_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "revoked",
        Some(auth.user_id),
        &assignment,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// PERMISSION ENDPOINTS
// =============================================================================

/// List all permissions
#[utoipa::path(
    get,
    path = "/rbac/permissions",
    tag = "RBAC",
    responses(
        (status = 200, description = "List of permissions", body = Vec<Permission>),
    ),
    security(("bearerAuth" = []))
)]
async fn list_permissions(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Permission>>, AppError> {
    use crate::db::uuid_sql::case_uuid;
    let id_case = case_uuid("id");
    let sql = format!("SELECT {}, name, description, created_at, updated_at FROM permissions ORDER BY name", id_case);

    let rows = sqlx::query(&sql)
    .fetch_all(&state.pool)
    .await?;

    let permissions: Vec<Permission> = rows.iter().map(|r| Permission {
        id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
        name: r.get("name"),
        description: r.get("description"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }).collect();

    Ok(Json(permissions))
}

/// Create a new permission
#[utoipa::path(
    post,
    path = "/rbac/permissions",
    tag = "RBAC",
    request_body = PermissionCreateRequest,
    responses(
        (status = 201, description = "Permission created", body = Permission),
        (status = 409, description = "Permission name already exists"),
    ),
    security(("bearerAuth" = []))
)]
async fn create_permission(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(req): Json<PermissionCreateRequest>,
) -> Result<(StatusCode, Json<Permission>), AppError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO permissions (id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(id.to_string())
    .bind(&req.name)
    .bind(&req.description)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let permission = Permission {
        id,
        name: req.name,
        description: req.description,
        created_at: now,
        updated_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "created",
        Some(auth.user_id),
        &permission,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok((StatusCode::CREATED, Json(permission)))
}

// =============================================================================
// USER-ROLE ENDPOINTS
// =============================================================================

/// Get roles assigned to a user
#[utoipa::path(
    get,
    path = "/rbac/users/{user_id}/roles",
    tag = "RBAC",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "List of assigned roles", body = Vec<Role>),
    ),
    security(("bearerAuth" = []))
)]
async fn get_user_roles(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Role>>, AppError> {
    use crate::db::uuid_sql::{case_uuid, match_uuid_clause};
    let id_case = case_uuid("r.id");
    let user_match = match_uuid_clause("ur.user_id");

    let sql = format!(
        r#"
        SELECT {}, r.name, r.description, r.created_at, r.updated_at
        FROM roles r
        INNER JOIN user_roles ur ON r.id = ur.role_id
        WHERE {}
        ORDER BY r.name
        "#,
        id_case, user_match
    );

    let rows = sqlx::query(&sql)
    .bind(user_id.to_string())
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    let roles: Vec<Role> = rows.iter().map(|r| Role {
        id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
        name: r.get("name"),
        description: r.get("description"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }).collect();

    Ok(Json(roles))
}

/// Assign a role to a user
#[utoipa::path(
    post,
    path = "/rbac/users/{user_id}/roles",
    tag = "RBAC",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    request_body = AssignRoleRequest,
    responses(
        (status = 201, description = "Role assigned"),
    ),
    security(("bearerAuth" = []))
)]
async fn assign_role_to_user(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(req): Json<AssignRoleRequest>,
) -> Result<StatusCode, AppError> {
    use crate::db::uuid_sql::match_uuid_clause;
    let now = Utc::now();

    let user_match = match_uuid_clause("user_id");
    let role_match = match_uuid_clause("role_id");
    let check_sql = format!("SELECT 1 FROM user_roles WHERE {} AND {}", user_match, role_match);

    let existing = sqlx::query(&check_sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(req.role_id.to_string())
        .bind(req.role_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if existing.is_none() {
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, created_at) VALUES (?, ?, ?)"
        )
        .bind(user_id.to_string())
        .bind(req.role_id.to_string())
        .bind(now)
        .execute(&state.pool)
        .await?;
    }

    let assignment = UserRole {
        user_id,
        role_id: req.role_id,
        created_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "assigned",
        Some(auth.user_id),
        &assignment,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok(StatusCode::CREATED)
}

/// Revoke a role from a user
#[utoipa::path(
    delete,
    path = "/rbac/users/{user_id}/roles/{role_id}",
    tag = "RBAC",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ("role_id" = Uuid, Path, description = "Role ID"),
    ),
    responses(
        (status = 204, description = "Role revoked"),
    ),
    security(("bearerAuth" = []))
)]
async fn revoke_role_from_user(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path((user_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    use crate::db::uuid_sql::match_uuid_clause;
    let now = Utc::now();

    let user_match = match_uuid_clause("user_id");
    let role_match = match_uuid_clause("role_id");
    let sql = format!("DELETE FROM user_roles WHERE {} AND {}", user_match, role_match);

    sqlx::query(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(role_id.to_string())
        .bind(role_id.to_string())
        .execute(&state.pool)
        .await?;

    let assignment = UserRole {
        user_id,
        role_id,
        created_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "revoked",
        Some(auth.user_id),
        &assignment,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// USER-PERMISSION ENDPOINTS
// =============================================================================

/// Get direct permissions granted to a user
#[utoipa::path(
    get,
    path = "/rbac/users/{user_id}/permissions",
    tag = "RBAC",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "List of direct permissions", body = Vec<UserPermission>),
    ),
    security(("bearerAuth" = []))
)]
async fn get_user_permissions(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<UserPermission>>, AppError> {
    use crate::db::uuid_sql::{case_uuid, match_uuid_clause};
    let id_case = case_uuid("id");
    let u_case = case_uuid("user_id");
    let p_case = case_uuid("permission_id");
    let user_match = match_uuid_clause("user_id");

    let sql = format!(
        r#"
        SELECT {}, {}, {}, scope, created_at
        FROM user_permissions
        WHERE {}
        "#,
        id_case, u_case, p_case, user_match
    );

    let rows = sqlx::query(&sql)
    .bind(user_id.to_string())
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    let permissions: Vec<UserPermission> = rows.iter().map(|r| {
        let scope_str: Option<String> = r.get("scope");
        UserPermission {
            id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            user_id: Uuid::parse_str(r.get::<&str, _>("user_id")).unwrap_or_default(),
            permission_id: Uuid::parse_str(r.get::<&str, _>("permission_id")).unwrap_or_default(),
            scope: scope_str.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Object(Default::default())),
            created_at: r.get("created_at"),
        }
    }).collect();

    Ok(Json(permissions))
}

/// Grant a permission directly to a user
#[utoipa::path(
    post,
    path = "/rbac/users/{user_id}/permissions",
    tag = "RBAC",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    request_body = GrantPermissionRequest,
    responses(
        (status = 201, description = "Permission granted"),
    ),
    security(("bearerAuth" = []))
)]
async fn grant_permission_to_user(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(req): Json<GrantPermissionRequest>,
) -> Result<StatusCode, AppError> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let scope_val = req.scope.clone().unwrap_or(Value::Object(Default::default()));
    let scope_str = serde_json::to_string(&scope_val)
        .map_err(|e| AppError::bad_request(format!("Invalid scope JSON: {}", e)))?;

    sqlx::query(
        "INSERT INTO user_permissions (id, user_id, permission_id, scope, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(id.to_string())
    .bind(user_id.to_string())
    .bind(req.permission_id.to_string())
    .bind(&scope_str)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let grant = UserPermission {
        id,
        user_id,
        permission_id: req.permission_id,
        scope: scope_val,
        created_at: now,
    };

    log_activity_with_context(
        &state.event_bus,
        "granted",
        Some(auth.user_id),
        &grant,
        None,
        Some(RequestContext::from_headers(&headers)),
    );

    Ok(StatusCode::CREATED)
}

// =============================================================================
// EFFECTIVE PERMISSIONS
// =============================================================================

/// Get computed effective permissions for a user
#[utoipa::path(
    get,
    path = "/rbac/users/{user_id}/effective-permissions",
    tag = "RBAC",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Effective permissions", body = EffectivePermissions),
    ),
    security(("bearerAuth" = []))
)]
async fn get_effective_permissions(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<Json<EffectivePermissions>, AppError> {
    use crate::db::uuid_sql::match_uuid_clause;

    // Fetch user's roles
    let user_match = match_uuid_clause("ur.user_id");
    let role_sql = format!(
        r#"
        SELECT r.name
        FROM roles r
        INNER JOIN user_roles ur ON r.id = ur.role_id
        WHERE {}
        "#,
        user_match
    );
    let role_rows = sqlx::query(&role_sql)
    .bind(user_id.to_string())
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    let roles: Vec<String> = role_rows.iter().map(|r| r.get("name")).collect();

    // Fetch role permissions
    let role_perm_sql = format!(
        r#"
        SELECT p.name as permission_name, r.name as role_name
        FROM permissions p
        INNER JOIN role_permissions rp ON p.id = rp.permission_id
        INNER JOIN roles r ON r.id = rp.role_id
        INNER JOIN user_roles ur ON r.id = ur.role_id
        WHERE {}
        "#,
        user_match
    );
    let role_perm_rows = sqlx::query(&role_perm_sql)
    .bind(user_id.to_string())
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    // Fetch direct permissions
    let direct_match = match_uuid_clause("up.user_id");
    let direct_sql = format!(
        r#"
        SELECT p.name, up.scope
        FROM permissions p
        INNER JOIN user_permissions up ON p.id = up.permission_id
        WHERE {}
        "#,
        direct_match
    );
    let direct_perm_rows = sqlx::query(&direct_sql)
    .bind(user_id.to_string())
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    let mut permissions: Vec<EffectivePermission> = Vec::new();

    // Add role permissions
    for p in role_perm_rows {
        permissions.push(EffectivePermission {
            name: p.get("permission_name"),
            source: "role".to_string(),
            role_name: Some(p.get("role_name")),
            scope: None,
        });
    }

    // Add direct permissions
    for p in direct_perm_rows {
        let scope_str: Option<String> = p.get("scope");
        let scope = scope_str.and_then(|s| serde_json::from_str(&s).ok());
        permissions.push(EffectivePermission {
            name: p.get("name"),
            source: "direct".to_string(),
            role_name: None,
            scope,
        });
    }

    Ok(Json(EffectivePermissions {
        user_id,
        roles,
        permissions,
    }))
}
