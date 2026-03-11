use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get},
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
use crate::models::audit_log::{AuditLogEntry, AuditLogFilter, PaginatedAuditLogs};
use crate::models::rbac::*;

// =============================================================================
// ROUTER
// =============================================================================

pub fn routes(_state: AppState) -> Router<AppState> {
    Router::new()
        // Roles
        .route("/roles", get(list_roles).post(create_role))
        .route("/roles/:role_id", get(get_role).delete(delete_role))
        .route(
            "/roles/:role_id/permissions",
            get(get_role_permissions).post(assign_permission_to_role),
        )
        .route(
            "/roles/:role_id/permissions/:permission_id",
            delete(delete_permission_from_role),
        )
        // Permissions
        .route(
            "/permissions",
            get(list_permissions).post(create_permission),
        )
        // User role assignments
        .route(
            "/users/:user_id/roles",
            get(get_user_roles).post(assign_role_to_user),
        )
        .route(
            "/users/:user_id/roles/:role_id",
            delete(revoke_role_from_user),
        )
        // User direct permissions
        .route(
            "/users/:user_id/permissions",
            get(get_user_permissions).post(grant_permission_to_user),
        )
        // Effective permissions (computed)
        .route(
            "/users/:user_id/effective-permissions",
            get(get_effective_permissions),
        )
        // Audit logs
        .route("/audit-logs", get(list_audit_logs))
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
    let sql = format!(
        "SELECT {}, name, description, created_at, updated_at FROM roles ORDER BY name",
        id_case
    );

    let rows = sqlx::query(&sql).fetch_all(&state.pool).await?;

    let roles: Vec<Role> = rows
        .iter()
        .map(|r| Role {
            id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            name: r.get("name"),
            description: r.get("description"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

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
        "INSERT INTO roles (id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
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
    let sql = format!(
        "SELECT {}, name, description, created_at, updated_at FROM roles WHERE {}",
        id_case, match_clause
    );

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
    let fetch_sql = format!(
        "SELECT {}, name, description, created_at, updated_at FROM roles WHERE {}",
        id_case, match_clause
    );

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
    let check_sql = format!(
        "SELECT 1 FROM role_permissions WHERE {} AND {}",
        role_match, perm_match
    );
    let existing = sqlx::query(&check_sql)
        .bind(role_id.to_string())
        .bind(role_id.to_string())
        .bind(req.permission_id.to_string())
        .bind(req.permission_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if existing.is_none() {
        let match_role = match_uuid_clause("id");
        let match_perm = match_uuid_clause("id");
        let insert_sql = format!(
            "INSERT INTO role_permissions (role_id, permission_id, created_at) VALUES ((SELECT id FROM roles WHERE {}), (SELECT id FROM permissions WHERE {}), ?)",
            match_role, match_perm
        );
        sqlx::query(&insert_sql)
            .bind(role_id.to_string())
            .bind(role_id.to_string())
            .bind(req.permission_id.to_string())
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

    let permissions: Vec<Permission> = rows
        .iter()
        .map(|r| Permission {
            id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            name: r.get("name"),
            description: r.get("description"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

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
    let sql = format!(
        "DELETE FROM role_permissions WHERE {} AND {}",
        role_match, perm_match
    );

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
    let sql = format!(
        "SELECT {}, name, description, created_at, updated_at FROM permissions ORDER BY name",
        id_case
    );

    let rows = sqlx::query(&sql).fetch_all(&state.pool).await?;

    let permissions: Vec<Permission> = rows
        .iter()
        .map(|r| Permission {
            id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            name: r.get("name"),
            description: r.get("description"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

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

    let roles: Vec<Role> = rows
        .iter()
        .map(|r| Role {
            id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            name: r.get("name"),
            description: r.get("description"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

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
    let check_sql = format!(
        "SELECT 1 FROM user_roles WHERE {} AND {}",
        user_match, role_match
    );

    let existing = sqlx::query(&check_sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(req.role_id.to_string())
        .bind(req.role_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if existing.is_none() {
        let match_user = match_uuid_clause("id");
        let match_role = match_uuid_clause("id");
        let insert_sql = format!(
            "INSERT INTO user_roles (user_id, role_id, created_at) VALUES ((SELECT id FROM users WHERE {}), (SELECT id FROM roles WHERE {}), ?)",
            match_user, match_role
        );
        sqlx::query(&insert_sql)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(req.role_id.to_string())
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
    let sql = format!(
        "DELETE FROM user_roles WHERE {} AND {}",
        user_match, role_match
    );

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

    let permissions: Vec<UserPermission> = rows
        .iter()
        .map(|r| {
            let scope_str: Option<String> = r.get("scope");
            UserPermission {
                id: Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
                user_id: Uuid::parse_str(r.get::<&str, _>("user_id")).unwrap_or_default(),
                permission_id: Uuid::parse_str(r.get::<&str, _>("permission_id"))
                    .unwrap_or_default(),
                scope: scope_str
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or(Value::Object(Default::default())),
                created_at: r.get("created_at"),
            }
        })
        .collect();

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
    let scope_val = req
        .scope
        .clone()
        .unwrap_or(Value::Object(Default::default()));
    let scope_str = serde_json::to_string(&scope_val)
        .map_err(|e| AppError::bad_request(format!("Invalid scope JSON: {}", e)))?;

    use crate::db::uuid_sql::match_uuid_clause;
    let match_user = match_uuid_clause("id");
    let match_perm = match_uuid_clause("id");
    let insert_sql = format!(
        "INSERT INTO user_permissions (id, user_id, permission_id, scope, created_at) VALUES (?, (SELECT id FROM users WHERE {}), (SELECT id FROM permissions WHERE {}), ?, ?)",
        match_user, match_perm
    );

    sqlx::query(&insert_sql)
        .bind(id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(req.permission_id.to_string())
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

// =============================================================================
// AUDIT LOG ENDPOINT
// =============================================================================

#[utoipa::path(
    get,
    path = "/rbac/audit-logs",
    tag = "RBAC",
    params(AuditLogFilter),
    responses(
        (status = 200, description = "Paginated audit log entries", body = PaginatedAuditLogs,
         headers(
            ("X-Total-Count" = i64, description = "Total number of matching entries")
         )
        )
    ),
    security(("bearerAuth" = []))
)]
pub async fn list_audit_logs(
    State(state): State<AppState>,
    _auth: AuthUser,
    axum::extract::Query(filter): axum::extract::Query<AuditLogFilter>,
) -> Result<(axum::http::HeaderMap, Json<PaginatedAuditLogs>), AppError> {
    use crate::db::uuid_sql;

    // Clamp per_page to max 100
    let per_page = filter.per_page.clamp(1, 100);
    let page = filter.page.max(1);
    let offset = (page - 1) * per_page;

    // Build dynamic WHERE clause
    let mut conditions = vec!["1=1".to_string()];
    let mut bind_values: Vec<String> = vec![];

    if let Some(ref action) = filter.action {
        conditions.push("event_name = ?".to_string());
        bind_values.push(action.clone());
    }

    if let Some(ref user_id) = filter.user_id {
        conditions.push(format!("({})", uuid_sql::match_uuid_clause("subject_id")));
        bind_values.push(user_id.to_string());
        bind_values.push(user_id.to_string());
    }

    if let Some(ref actor_id) = filter.actor_id {
        conditions.push(format!("({})", uuid_sql::match_uuid_clause("actor_id")));
        bind_values.push(actor_id.to_string());
        bind_values.push(actor_id.to_string());
    }

    if let Some(ref from) = filter.from {
        conditions.push("occurred_at >= ?".to_string());
        bind_values.push(from.to_rfc3339());
    }

    if let Some(ref to) = filter.to {
        conditions.push("occurred_at <= ?".to_string());
        bind_values.push(to.to_rfc3339());
    }

    let where_clause = conditions.join(" AND ");

    // Count total
    let count_sql = format!("SELECT COUNT(*) FROM activity_log WHERE {}", where_clause);
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    for val in &bind_values {
        count_query = count_query.bind(val);
    }
    let total: i64 = count_query.fetch_one(&state.pool).await?;

    // Fetch page
    let select_sql = format!(
        "SELECT id, event_name, actor_id, subject_id, properties, occurred_at FROM activity_log WHERE {} ORDER BY occurred_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let mut select_query = sqlx::query(&select_sql);
    for val in &bind_values {
        select_query = select_query.bind(val);
    }
    select_query = select_query.bind(per_page).bind(offset);

    let rows = select_query.fetch_all(&state.pool).await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id").unwrap_or_default();
        let action: String = row.try_get("event_name").unwrap_or_default();
        let actor_id_str: Option<String> = row.try_get("actor_id").ok();
        let subject_id_str: Option<String> = row.try_get("subject_id").ok();
        let properties: Option<String> = row.try_get("properties").ok();
        let occurred_at: chrono::DateTime<chrono::Utc> = row
            .try_get("occurred_at")
            .unwrap_or_else(|_| chrono::Utc::now());

        let actor_id = actor_id_str
            .as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok());
        let target_user_id = subject_id_str
            .as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok());

        // Parse properties JSON for details
        let details: serde_json::Value = properties
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null);

        items.push(AuditLogEntry {
            id,
            action,
            actor_id,
            actor_name: None, // TODO: Join with users table for names
            target_user_id,
            target_user_name: None,
            details,
            created_at: occurred_at,
        });
    }

    let mut headers = axum::http::HeaderMap::new();
    headers.insert("X-Total-Count", total.to_string().parse().unwrap());

    Ok((
        headers,
        Json(PaginatedAuditLogs {
            items,
            total,
            page,
            per_page,
        }),
    ))
}
