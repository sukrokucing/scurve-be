//! RBAC Admin API Routes
//!
//! Endpoints for managing roles, permissions, and user assignments.
//! All RBAC modifications are logged to the activity log with Critical severity.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, delete},
    Json, Router,
};
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

pub fn routes() -> Router<AppState> {
    Router::new()
        // Roles
        // Roles
        .route("/roles", get(list_roles).post(create_role))
        .route("/roles/:role_id", get(get_role).delete(delete_role))
        .route("/roles/:role_id/permissions", get(get_role_permissions).post(assign_permission_to_role))
        .route(
            "/roles/:role_id/permissions/:permission_id",
            delete(delete_permission_from_role),
        )
        // Permissions
        .route("/permissions", get(list_permissions).post(create_permission))
        // User role assignments
        .route("/users/:user_id/roles", get(get_user_roles).post(assign_role_to_user))
        .route("/users/:user_id/roles/:role_id", delete(revoke_role_from_user))
        // User direct permissions
        .route("/users/:user_id/permissions", get(get_user_permissions).post(grant_permission_to_user))
        // Effective permissions (computed)
        .route("/users/:user_id/effective-permissions", get(get_effective_permissions))
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
    let rows = sqlx::query(
        "SELECT id, name, description, created_at, updated_at FROM roles ORDER BY name"
    )
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
    let row = sqlx::query(
        "SELECT id, name, description, created_at, updated_at FROM roles WHERE id = ?"
    )
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
    let row = sqlx::query(
        "SELECT id, name, description, created_at, updated_at FROM roles WHERE id = ?"
    )
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

    sqlx::query("DELETE FROM roles WHERE id = ?")
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
    let now = Utc::now();

    sqlx::query(
        "INSERT OR IGNORE INTO role_permissions (role_id, permission_id, created_at) VALUES (?, ?, ?)"
    )
    .bind(role_id.to_string())
    .bind(req.permission_id.to_string())
    .bind(now)
    .execute(&state.pool)
    .await?;

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
    let rows = sqlx::query(
        r#"
        SELECT p.id, p.name, p.description, p.created_at, p.updated_at
        FROM permissions p
        INNER JOIN role_permissions rp ON p.id = rp.permission_id
        WHERE rp.role_id = ?
        ORDER BY p.name
        "#
    )
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
    let now = Utc::now();

    sqlx::query("DELETE FROM role_permissions WHERE role_id = ? AND permission_id = ?")
        .bind(role_id.to_string())
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
    let rows = sqlx::query(
        "SELECT id, name, description, created_at, updated_at FROM permissions ORDER BY name"
    )
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
    let rows = sqlx::query(
        r#"
        SELECT r.id, r.name, r.description, r.created_at, r.updated_at
        FROM roles r
        INNER JOIN user_roles ur ON r.id = ur.role_id
        WHERE ur.user_id = ?
        ORDER BY r.name
        "#
    )
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
    let now = Utc::now();

    sqlx::query(
        "INSERT OR IGNORE INTO user_roles (user_id, role_id, created_at) VALUES (?, ?, ?)"
    )
    .bind(user_id.to_string())
    .bind(req.role_id.to_string())
    .bind(now)
    .execute(&state.pool)
    .await?;

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
    let now = Utc::now();

    sqlx::query("DELETE FROM user_roles WHERE user_id = ? AND role_id = ?")
        .bind(user_id.to_string())
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
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, permission_id, scope, created_at
        FROM user_permissions
        WHERE user_id = ?
        "#
    )
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
    // Fetch user's roles
    let role_rows = sqlx::query(
        r#"
        SELECT r.name
        FROM roles r
        INNER JOIN user_roles ur ON r.id = ur.role_id
        WHERE ur.user_id = ?
        "#
    )
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    let roles: Vec<String> = role_rows.iter().map(|r| r.get("name")).collect();

    // Fetch role permissions
    let role_perm_rows = sqlx::query(
        r#"
        SELECT p.name as permission_name, r.name as role_name
        FROM permissions p
        INNER JOIN role_permissions rp ON p.id = rp.permission_id
        INNER JOIN roles r ON r.id = rp.role_id
        INNER JOIN user_roles ur ON r.id = ur.role_id
        WHERE ur.user_id = ?
        "#
    )
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await?;

    // Fetch direct permissions
    let direct_perm_rows = sqlx::query(
        r#"
        SELECT p.name, up.scope
        FROM permissions p
        INNER JOIN user_permissions up ON p.id = up.permission_id
        WHERE up.user_id = ?
        "#
    )
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
