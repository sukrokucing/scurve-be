use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::Row;
use uuid::Uuid;

use crate::app::AppState;
use crate::db::uuid_sql;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::resource_role::{
    ProjectResourceRoleRate, ProjectResourceRoleRateUpsertRequest, ResourceRole,
    ResourceRoleCreateRequest, ResourceRoleUpdateRequest,
};
use crate::utils::{parse_db_datetime, utc_now};

fn normalize_currency(raw: &str) -> AppResult<String> {
    let normalized = raw.trim().to_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::bad_request(
            "currency must be a 3-letter ISO code (e.g. USD)",
        ));
    }
    Ok(normalized)
}

async fn ensure_project_access(state: &AppState, user_id: Uuid, project_id: Uuid) -> AppResult<()> {
    let match_id = uuid_sql::match_uuid_clause("p.id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let member_match = uuid_sql::match_uuid_clause("pm.user_id");
    let sql = format!(
        "SELECT 1 FROM projects p
         WHERE {} AND p.deleted_at IS NULL
           AND (
               {}
               OR EXISTS (
                    SELECT 1 FROM project_members pm
                    WHERE pm.project_id = p.id
                      AND {}
                      AND pm.deleted_at IS NULL
               )
           )
         LIMIT 1",
        match_id, match_owner, member_match
    );

    let found: Option<i64> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if found.is_none() {
        return Err(AppError::not_found("project not found"));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/resource-roles",
    tag = "Projects",
    responses((status = 200, description = "List global resource roles", body = [ResourceRole])),
    security(("bearerAuth" = []))
)]
pub async fn list_resource_roles(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> AppResult<Json<Vec<ResourceRole>>> {
    let id_case = uuid_sql::case_uuid("id");
    let rows = sqlx::query(&format!(
        "SELECT {} , name, description, default_hourly_rate, currency, created_at, updated_at
         FROM resource_roles
         WHERE deleted_at IS NULL
         ORDER BY name ASC",
        id_case
    ))
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;
        out.push(ResourceRole {
            id: Uuid::parse_str(&id)
                .map_err(|e| AppError::internal(format!("invalid role id: {}", e)))?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            default_hourly_rate: row.try_get("default_hourly_rate")?,
            currency: row.try_get("currency")?,
            created_at: parse_db_datetime(&created_at)?,
            updated_at: parse_db_datetime(&updated_at)?,
        });
    }

    Ok(Json(out))
}

#[utoipa::path(
    post,
    path = "/resource-roles",
    tag = "Projects",
    request_body = ResourceRoleCreateRequest,
    responses((status = 201, description = "Resource role created", body = ResourceRole)),
    security(("bearerAuth" = []))
)]
pub async fn create_resource_role(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(payload): Json<ResourceRoleCreateRequest>,
) -> AppResult<(StatusCode, Json<ResourceRole>)> {
    let name = payload.name.trim().to_lowercase();
    if name.is_empty() {
        return Err(AppError::bad_request("name is required"));
    }
    if payload.default_hourly_rate < 0.0 {
        return Err(AppError::bad_request(
            "default_hourly_rate must be non-negative",
        ));
    }
    let currency = normalize_currency(&payload.currency)?;

    let id = Uuid::new_v4();
    let now = utc_now();

    sqlx::query(
        "INSERT INTO resource_roles (id, name, description, default_hourly_rate, currency, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(&name)
    .bind(payload.description)
    .bind(payload.default_hourly_rate)
    .bind(&currency)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let role = fetch_resource_role(&state, id).await?;
    Ok((StatusCode::CREATED, Json(role)))
}

#[utoipa::path(
    put,
    path = "/resource-roles/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Resource role id")),
    request_body = ResourceRoleUpdateRequest,
    responses((status = 200, description = "Resource role updated", body = ResourceRole)),
    security(("bearerAuth" = []))
)]
pub async fn update_resource_role(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ResourceRoleUpdateRequest>,
) -> AppResult<Json<ResourceRole>> {
    let mut role = fetch_resource_role(&state, id).await?;

    if let Some(name) = payload.name {
        let normalized = name.trim().to_lowercase();
        if normalized.is_empty() {
            return Err(AppError::bad_request("name must not be empty"));
        }
        role.name = normalized;
    }
    if let Some(description) = payload.description {
        role.description = if description.trim().is_empty() {
            None
        } else {
            Some(description)
        };
    }
    if let Some(default_hourly_rate) = payload.default_hourly_rate {
        if default_hourly_rate < 0.0 {
            return Err(AppError::bad_request(
                "default_hourly_rate must be non-negative",
            ));
        }
        role.default_hourly_rate = default_hourly_rate;
    }
    if let Some(currency) = payload.currency {
        role.currency = normalize_currency(&currency)?;
    }

    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    sqlx::query(&format!(
        "UPDATE resource_roles
         SET name = ?, description = ?, default_hourly_rate = ?, currency = ?, updated_at = ?
         WHERE {} AND deleted_at IS NULL",
        match_id
    ))
    .bind(&role.name)
    .bind(&role.description)
    .bind(role.default_hourly_rate)
    .bind(&role.currency)
    .bind(now)
    .bind(id.to_string())
    .bind(id.to_string())
    .execute(&state.pool)
    .await?;

    let updated = fetch_resource_role(&state, id).await?;
    Ok(Json(updated))
}

#[utoipa::path(
    delete,
    path = "/resource-roles/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Resource role id")),
    responses((status = 204, description = "Resource role soft deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_resource_role(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    let affected = sqlx::query(&format!(
        "UPDATE resource_roles SET deleted_at = ?, updated_at = ? WHERE {} AND deleted_at IS NULL",
        match_id
    ))
    .bind(now)
    .bind(now)
    .bind(id.to_string())
    .bind(id.to_string())
    .execute(&state.pool)
    .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("resource role not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/resource-roles",
    tag = "Projects",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List project effective resource role rates", body = [ProjectResourceRoleRate])),
    security(("bearerAuth" = []))
)]
pub async fn list_project_resource_roles(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<ProjectResourceRoleRate>>> {
    ensure_project_access(&state, auth.user_id, project_id).await?;

    let role_case = uuid_sql::case_uuid("rr.id");
    let match_proj = uuid_sql::match_uuid_clause("prr.project_id");
    let sql = format!(
        "SELECT
            {} ,
            rr.name AS role_name,
            COALESCE(prr.hourly_rate, rr.default_hourly_rate) AS hourly_rate,
            COALESCE(prr.currency, rr.currency) AS currency,
            CASE WHEN prr.id IS NULL THEN 0 ELSE 1 END AS is_override
         FROM resource_roles rr
         LEFT JOIN project_resource_role_rates prr
           ON prr.resource_role_id = rr.id
          AND {}
          AND prr.deleted_at IS NULL
         WHERE rr.deleted_at IS NULL
         ORDER BY rr.name ASC",
        role_case, match_proj
    );

    let rows = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let resource_role_id: String = row.try_get("id")?;
        let is_override: i64 = row.try_get("is_override")?;
        out.push(ProjectResourceRoleRate {
            resource_role_id: Uuid::parse_str(&resource_role_id)
                .map_err(|e| AppError::internal(format!("invalid role id: {}", e)))?,
            resource_role_name: row.try_get("role_name")?,
            hourly_rate: row.try_get("hourly_rate")?,
            currency: row.try_get("currency")?,
            is_override: is_override == 1,
        });
    }

    Ok(Json(out))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/resource-roles/{resource_role_id}/rate",
    tag = "Projects",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("resource_role_id" = Uuid, Path, description = "Resource role id")
    ),
    request_body = ProjectResourceRoleRateUpsertRequest,
    responses((status = 200, description = "Project resource role rate upserted", body = ProjectResourceRoleRate)),
    security(("bearerAuth" = []))
)]
pub async fn upsert_project_resource_role_rate(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, resource_role_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<ProjectResourceRoleRateUpsertRequest>,
) -> AppResult<Json<ProjectResourceRoleRate>> {
    ensure_project_access(&state, auth.user_id, project_id).await?;

    if payload.hourly_rate < 0.0 {
        return Err(AppError::bad_request("hourly_rate must be non-negative"));
    }
    let currency = normalize_currency(&payload.currency)?;

    let role_match = uuid_sql::match_uuid_clause("id");
    let role_exists: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1 FROM resource_roles WHERE {} AND deleted_at IS NULL",
        role_match
    ))
    .bind(resource_role_id.to_string())
    .bind(resource_role_id.to_string())
    .fetch_optional(&state.pool)
    .await?;
    if role_exists.is_none() {
        return Err(AppError::not_found("resource role not found"));
    }

    let now = utc_now();
    let match_project = uuid_sql::match_uuid_clause("project_id");
    let match_role = uuid_sql::match_uuid_clause("resource_role_id");
    let existing_sql = format!(
        "SELECT id FROM project_resource_role_rates
         WHERE {} AND {} AND deleted_at IS NULL
         LIMIT 1",
        match_project, match_role
    );

    let existing: Option<String> = sqlx::query_scalar(&existing_sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(resource_role_id.to_string())
        .bind(resource_role_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if let Some(id) = existing {
        let match_id = uuid_sql::match_uuid_clause("id");
        let actor_match = uuid_sql::match_uuid_clause("id");
        let update_sql = format!(
            "UPDATE project_resource_role_rates
             SET hourly_rate = ?,
                 currency = ?,
                 updated_at = ?,
                 updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
             WHERE {}",
            actor_match, match_id
        );
        sqlx::query(&update_sql)
            .bind(payload.hourly_rate)
            .bind(&currency)
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .bind(id.clone())
            .bind(id)
            .execute(&state.pool)
            .await?;
    } else {
        let project_match = uuid_sql::match_uuid_clause("id");
        let role_match = uuid_sql::match_uuid_clause("id");
        let actor_match = uuid_sql::match_uuid_clause("id");
        let insert_sql = format!(
            "INSERT INTO project_resource_role_rates (
                id, project_id, resource_role_id, hourly_rate, currency,
                created_at, created_by, updated_at, updated_by
             ) VALUES (
                ?,
                (SELECT id FROM projects WHERE {} AND deleted_at IS NULL),
                (SELECT id FROM resource_roles WHERE {} AND deleted_at IS NULL),
                ?,
                ?,
                ?,
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
                ?,
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
             )",
            project_match, role_match, actor_match, actor_match
        );

        sqlx::query(&insert_sql)
            .bind(Uuid::new_v4().to_string())
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .bind(resource_role_id.to_string())
            .bind(resource_role_id.to_string())
            .bind(payload.hourly_rate)
            .bind(&currency)
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .execute(&state.pool)
            .await?;
    }

    let list = list_project_resource_roles(State(state.clone()), auth, Path(project_id))
        .await?
        .0;

    let item = list
        .into_iter()
        .find(|it| it.resource_role_id == resource_role_id)
        .ok_or_else(|| {
            AppError::internal("project role rate not found after upsert".to_string())
        })?;

    Ok(Json(item))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/resource-roles/{resource_role_id}/rate",
    tag = "Projects",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("resource_role_id" = Uuid, Path, description = "Resource role id")
    ),
    responses((status = 204, description = "Project resource role rate deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_project_resource_role_rate(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, resource_role_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    ensure_project_access(&state, auth.user_id, project_id).await?;

    let now = utc_now();
    let match_project = uuid_sql::match_uuid_clause("project_id");
    let match_role = uuid_sql::match_uuid_clause("resource_role_id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "UPDATE project_resource_role_rates
         SET deleted_at = ?,
             deleted_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
             updated_at = ?,
             updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         WHERE {} AND {} AND deleted_at IS NULL",
        actor_match, actor_match, match_project, match_role
    );

    let affected = sqlx::query(&sql)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(resource_role_id.to_string())
        .bind(resource_role_id.to_string())
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("project role rate not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_resource_role(state: &AppState, id: Uuid) -> AppResult<ResourceRole> {
    let id_case = uuid_sql::case_uuid("id");
    let match_id = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "SELECT {} , name, description, default_hourly_rate, currency, created_at, updated_at
         FROM resource_roles
         WHERE {} AND deleted_at IS NULL
         LIMIT 1",
        id_case, match_id
    );

    let row = sqlx::query(&sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("resource role not found"))?;

    let role_id: String = row.try_get("id")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    Ok(ResourceRole {
        id: Uuid::parse_str(&role_id)
            .map_err(|e| AppError::internal(format!("invalid role id: {}", e)))?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        default_hourly_rate: row.try_get("default_hourly_rate")?,
        currency: row.try_get("currency")?,
        created_at: parse_db_datetime(&created_at)?,
        updated_at: parse_db_datetime(&updated_at)?,
    })
}
