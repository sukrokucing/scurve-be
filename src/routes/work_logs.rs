use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::app::AppState;
use crate::authz::permissions;
use crate::authz::{resolve_data_user, DefaultPolicyEvaluator, PolicyEvaluator, Principal, ResourceContext, ViewAsContext};
use crate::db::uuid_sql;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::work_log::{WorkLog, WorkLogCreateRequest, WorkLogSource, WorkLogUpdateRequest};
use crate::utils::{parse_db_datetime, round2, utc_now};

fn parse_work_date(value: Option<&str>) -> AppResult<String> {
    let Some(raw) = value else {
        return Ok(Utc::now().date_naive().to_string());
    };
    let trimmed = raw.trim();
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(date.to_string());
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.date_naive().to_string());
    }
    Err(AppError::bad_request(
        "work_date must be YYYY-MM-DD or RFC3339 timestamp",
    ))
}

#[derive(Debug, Clone)]
struct StoredWorkLog {
    id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    user_id: Option<Uuid>,
    user_name: Option<String>,
    resource_role_id: Uuid,
    resource_role_name: String,
    hours: f64,
    hourly_rate_snapshot: f64,
    currency_snapshot: String,
    cost_amount: f64,
    work_date: String,
    note: Option<String>,
    source: WorkLogSource,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

impl From<StoredWorkLog> for WorkLog {
    fn from(value: StoredWorkLog) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            task_id: value.task_id,
            user_id: value.user_id,
            user_name: value.user_name,
            resource_role_id: value.resource_role_id,
            resource_role_name: value.resource_role_name,
            hours: value.hours,
            hourly_rate_snapshot: value.hourly_rate_snapshot,
            currency_snapshot: value.currency_snapshot,
            cost_amount: value.cost_amount,
            work_date: value.work_date,
            note: value.note,
            source: value.source,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        }
    }
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{task_id}/work-logs",
    tag = "Progress",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("task_id" = Uuid, Path, description = "Task id")
    ),
    responses((status = 200, description = "List work logs", body = [WorkLog])),
    security(("bearerAuth" = []))
)]
pub async fn list_work_logs(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
    view_as: Option<axum::Extension<ViewAsContext>>,
) -> AppResult<Json<Vec<WorkLog>>> {
    ensure_task_access(&state, resolve_data_user(&view_as, auth.user_id), project_id, task_id).await?;

    let rows = fetch_work_logs(&state, project_id, task_id).await?;
    Ok(Json(rows.into_iter().map(WorkLog::from).collect()))
}

#[utoipa::path(
    post,
    path = "/projects/{project_id}/tasks/{task_id}/work-logs",
    tag = "Progress",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("task_id" = Uuid, Path, description = "Task id")
    ),
    request_body = WorkLogCreateRequest,
    responses((status = 201, description = "Work log created", body = WorkLog)),
    security(("bearerAuth" = []))
)]
pub async fn create_work_log(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Json(payload): Json<WorkLogCreateRequest>,
) -> AppResult<(StatusCode, Json<WorkLog>)> {
    ensure_task_access(&state, auth.user_id, project_id, task_id).await?;

    if payload.hours <= 0.0 {
        return Err(AppError::bad_request("hours must be greater than 0"));
    }

    let target_user_id = payload.user_id.unwrap_or(auth.user_id);
    if target_user_id != auth.user_id
        && !has_project_update_permission(&state, auth.user_id, project_id).await?
    {
        return Err(AppError::forbidden(
            "logging work for another user requires project.update permission",
        ));
    }

    let membership_id = ensure_active_membership(&state, project_id, target_user_id).await?;
    ensure_membership_has_resource_role(&state, membership_id, payload.resource_role_id).await?;

    let (hourly_rate_snapshot, currency_snapshot) =
        resolve_rate_snapshot(&state, project_id, payload.resource_role_id).await?;
    let cost_amount = round2(payload.hours * hourly_rate_snapshot);
    let work_date = parse_work_date(payload.work_date.as_deref())?;

    let now = utc_now();
    let id = Uuid::new_v4();
    let project_match = uuid_sql::match_uuid_clause("id");
    let task_match = uuid_sql::match_uuid_clause("id");
    let user_match = uuid_sql::match_uuid_clause("id");
    let role_match = uuid_sql::match_uuid_clause("id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "INSERT INTO work_logs (
            id,
            project_id,
            task_id,
            user_id,
            resource_role_id,
            hours,
            hourly_rate_snapshot,
            currency_snapshot,
            cost_amount,
            work_date,
            note,
            source,
            created_at,
            created_by,
            updated_at,
            updated_by
         ) VALUES (
            ?,
            (SELECT id FROM projects WHERE {} AND deleted_at IS NULL),
            (SELECT id FROM tasks WHERE {} AND deleted_at IS NULL),
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
            (SELECT id FROM resource_roles WHERE {} AND deleted_at IS NULL),
            ?, ?, ?, ?, ?, ?, 'manual', ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
            ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         )",
        project_match, task_match, user_match, role_match, actor_match, actor_match
    );

    sqlx::query(&sql)
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .bind(target_user_id.to_string())
        .bind(target_user_id.to_string())
        .bind(payload.resource_role_id.to_string())
        .bind(payload.resource_role_id.to_string())
        .bind(payload.hours)
        .bind(hourly_rate_snapshot)
        .bind(&currency_snapshot)
        .bind(cost_amount)
        .bind(&work_date)
        .bind(payload.note)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .execute(&state.pool)
        .await?;

    let created = fetch_work_log(&state, project_id, task_id, id).await?;
    let created: WorkLog = created.into();

    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "created",
        Some(auth.user_id),
        &created,
        None,
        Some(ctx),
    );

    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/{task_id}/work-logs/{id}",
    tag = "Progress",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("task_id" = Uuid, Path, description = "Task id"),
        ("id" = Uuid, Path, description = "Work log id")
    ),
    request_body = WorkLogUpdateRequest,
    responses((status = 200, description = "Work log updated", body = WorkLog)),
    security(("bearerAuth" = []))
)]
pub async fn update_work_log(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Json(payload): Json<WorkLogUpdateRequest>,
) -> AppResult<Json<WorkLog>> {
    ensure_task_access(&state, auth.user_id, project_id, task_id).await?;

    let mut row = fetch_work_log(&state, project_id, task_id, id).await?;
    let old_item: WorkLog = row.clone().into();

    if row.user_id != Some(auth.user_id)
        && !has_project_update_permission(&state, auth.user_id, project_id).await?
    {
        return Err(AppError::forbidden(
            "updating another user's work log requires project.update permission",
        ));
    }

    if let Some(hours) = payload.hours {
        if hours <= 0.0 {
            return Err(AppError::bad_request("hours must be greater than 0"));
        }
        row.hours = hours;
    }

    if let Some(resource_role_id) = payload.resource_role_id {
        let user_id = row
            .user_id
            .ok_or_else(|| AppError::bad_request("work log has no user assigned"))?;
        let membership_id = ensure_active_membership(&state, project_id, user_id).await?;
        ensure_membership_has_resource_role(&state, membership_id, resource_role_id).await?;
        row.resource_role_id = resource_role_id;
    }

    if let Some(work_date) = payload.work_date {
        row.work_date = parse_work_date(Some(&work_date))?;
    }

    if payload.note.is_some() {
        row.note = payload.note;
    }

    let (hourly_rate_snapshot, currency_snapshot) =
        resolve_rate_snapshot(&state, project_id, row.resource_role_id).await?;
    row.hourly_rate_snapshot = hourly_rate_snapshot;
    row.currency_snapshot = currency_snapshot;
    row.cost_amount = round2(row.hours * row.hourly_rate_snapshot);

    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let role_match = uuid_sql::match_uuid_clause("id");

    let sql = format!(
        "UPDATE work_logs
         SET resource_role_id = (SELECT id FROM resource_roles WHERE {} AND deleted_at IS NULL),
             hours = ?,
             hourly_rate_snapshot = ?,
             currency_snapshot = ?,
             cost_amount = ?,
             work_date = ?,
             note = ?,
             updated_at = ?,
             updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         WHERE {} AND deleted_at IS NULL",
        role_match, actor_match, match_id
    );

    sqlx::query(&sql)
        .bind(row.resource_role_id.to_string())
        .bind(row.resource_role_id.to_string())
        .bind(row.hours)
        .bind(row.hourly_rate_snapshot)
        .bind(&row.currency_snapshot)
        .bind(row.cost_amount)
        .bind(&row.work_date)
        .bind(&row.note)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(id.to_string())
        .bind(id.to_string())
        .execute(&state.pool)
        .await?;

    let updated = fetch_work_log(&state, project_id, task_id, id).await?;
    let updated: WorkLog = updated.into();

    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "updated",
        Some(auth.user_id),
        &updated,
        Some(&old_item),
        Some(ctx),
    );

    Ok(Json(updated))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/tasks/{task_id}/work-logs/{id}",
    tag = "Progress",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("task_id" = Uuid, Path, description = "Task id"),
        ("id" = Uuid, Path, description = "Work log id")
    ),
    responses((status = 204, description = "Work log deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_work_log(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
) -> AppResult<StatusCode> {
    ensure_task_access(&state, auth.user_id, project_id, task_id).await?;

    let row = fetch_work_log(&state, project_id, task_id, id).await?;
    let old_item: WorkLog = row.clone().into();
    if row.user_id != Some(auth.user_id)
        && !has_project_update_permission(&state, auth.user_id, project_id).await?
    {
        return Err(AppError::forbidden(
            "deleting another user's work log requires project.update permission",
        ));
    }

    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "UPDATE work_logs
         SET deleted_at = ?,
             deleted_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
             updated_at = ?,
             updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         WHERE {} AND deleted_at IS NULL",
        actor_match, actor_match, match_id
    );

    let affected = sqlx::query(&sql)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(id.to_string())
        .bind(id.to_string())
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("work log not found"));
    }

    let mut deleted_item = old_item.clone();
    deleted_item.deleted_at = Some(now);
    deleted_item.updated_at = now;

    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "deleted",
        Some(auth.user_id),
        &deleted_item,
        Some(&old_item),
        Some(ctx),
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn ensure_task_access(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> AppResult<()> {
    let match_task = uuid_sql::match_uuid_clause("t.id");
    let match_project = uuid_sql::match_uuid_clause("t.project_id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let member_match = uuid_sql::match_uuid_clause("pm.user_id");

    let sql = format!(
        "SELECT 1
         FROM tasks t
         INNER JOIN projects p ON p.id = t.project_id
         WHERE {} AND {} AND t.deleted_at IS NULL AND p.deleted_at IS NULL
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
        match_task, match_project, match_owner, member_match
    );

    let found: Option<i64> = sqlx::query_scalar(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if found.is_none() {
        return Err(AppError::not_found("task not found"));
    }

    Ok(())
}

async fn ensure_active_membership(
    state: &AppState,
    project_id: Uuid,
    user_id: Uuid,
) -> AppResult<Uuid> {
    let match_project = uuid_sql::match_uuid_clause("project_id");
    let match_user = uuid_sql::match_uuid_clause("user_id");
    let id_case = uuid_sql::case_uuid("id");
    let sql = format!(
        "SELECT {} FROM project_members
         WHERE {} AND {} AND deleted_at IS NULL
         LIMIT 1",
        id_case, match_project, match_user
    );

    let membership_id: Option<String> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    let membership_id =
        membership_id.ok_or_else(|| AppError::forbidden("target user is not a project member"))?;
    Uuid::parse_str(&membership_id)
        .map_err(|e| AppError::internal(format!("invalid membership id: {}", e)))
}

async fn ensure_membership_has_resource_role(
    state: &AppState,
    membership_id: Uuid,
    resource_role_id: Uuid,
) -> AppResult<()> {
    let match_membership = uuid_sql::match_uuid_clause("pmrr.membership_id");
    let match_role = uuid_sql::match_uuid_clause("pmrr.resource_role_id");
    let sql = format!(
        "SELECT 1
         FROM project_member_resource_roles pmrr
         INNER JOIN resource_roles rr ON rr.id = pmrr.resource_role_id
         WHERE {} AND {} AND pmrr.deleted_at IS NULL AND rr.deleted_at IS NULL
         LIMIT 1",
        match_membership, match_role
    );

    let found: Option<i64> = sqlx::query_scalar(&sql)
        .bind(membership_id.to_string())
        .bind(membership_id.to_string())
        .bind(resource_role_id.to_string())
        .bind(resource_role_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    if found.is_none() {
        return Err(AppError::forbidden(
            "target user is not assigned to the submitted resource role",
        ));
    }

    Ok(())
}

async fn resolve_rate_snapshot(
    state: &AppState,
    project_id: Uuid,
    resource_role_id: Uuid,
) -> AppResult<(f64, String)> {
    let match_project = uuid_sql::match_uuid_clause("prr.project_id");
    let match_role = uuid_sql::match_uuid_clause("prr.resource_role_id");
    let sql = format!(
        "SELECT prr.hourly_rate, prr.currency
         FROM project_resource_role_rates prr
         WHERE {} AND {} AND prr.deleted_at IS NULL
         LIMIT 1",
        match_project, match_role
    );

    if let Some(row) = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(resource_role_id.to_string())
        .bind(resource_role_id.to_string())
        .fetch_optional(&state.pool)
        .await?
    {
        let rate: f64 = row.try_get("hourly_rate")?;
        let currency: String = row.try_get("currency")?;
        return Ok((rate, currency));
    }

    let match_role = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "SELECT default_hourly_rate, currency
         FROM resource_roles
         WHERE {} AND deleted_at IS NULL
         LIMIT 1",
        match_role
    );

    let row = sqlx::query(&sql)
        .bind(resource_role_id.to_string())
        .bind(resource_role_id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("resource role not found"))?;

    let rate: f64 = row.try_get("default_hourly_rate")?;
    let currency: String = row.try_get("currency")?;
    Ok((rate, currency))
}

async fn has_project_update_permission(
    state: &AppState,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<bool> {
    let principal = Principal::load(user_id, &state.pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to load principal: {}", e)))?;
    let ctx = ResourceContext::new().with_project(project_id);
    let evaluator = DefaultPolicyEvaluator::new();
    Ok(evaluator
        .can(&principal, permissions::PROJECT_UPDATE, &ctx)
        .await)
}

async fn fetch_work_logs(
    state: &AppState,
    project_id: Uuid,
    task_id: Uuid,
) -> AppResult<Vec<StoredWorkLog>> {
    let id_case = uuid_sql::case_uuid("wl.id");
    let project_case = uuid_sql::case_uuid("wl.project_id");
    let task_case = uuid_sql::case_uuid("wl.task_id");
    let user_case = uuid_sql::case_uuid("wl.user_id");
    let role_case = uuid_sql::case_uuid("wl.resource_role_id");
    let match_project = uuid_sql::match_uuid_clause("wl.project_id");
    let match_task = uuid_sql::match_uuid_clause("wl.task_id");

    let sql = format!(
        "SELECT
            {} ,
            {} ,
            {} ,
            {} ,
            u.name AS user_name,
            {} ,
            rr.name AS resource_role_name,
            wl.hours,
            wl.hourly_rate_snapshot,
            wl.currency_snapshot,
            wl.cost_amount,
            wl.work_date,
            wl.note,
            wl.source,
            wl.created_at,
            wl.updated_at,
            wl.deleted_at
         FROM work_logs wl
         LEFT JOIN users u ON u.id = wl.user_id
         INNER JOIN resource_roles rr ON rr.id = wl.resource_role_id
         WHERE {} AND {} AND wl.deleted_at IS NULL
         ORDER BY wl.work_date DESC, wl.created_at DESC",
        id_case, project_case, task_case, user_case, role_case, match_project, match_task
    );

    let rows = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(parse_work_log_row(&row)?);
    }
    Ok(out)
}

async fn fetch_work_log(
    state: &AppState,
    project_id: Uuid,
    task_id: Uuid,
    id: Uuid,
) -> AppResult<StoredWorkLog> {
    let id_case = uuid_sql::case_uuid("wl.id");
    let project_case = uuid_sql::case_uuid("wl.project_id");
    let task_case = uuid_sql::case_uuid("wl.task_id");
    let user_case = uuid_sql::case_uuid("wl.user_id");
    let role_case = uuid_sql::case_uuid("wl.resource_role_id");

    let match_id = uuid_sql::match_uuid_clause("wl.id");
    let match_project = uuid_sql::match_uuid_clause("wl.project_id");
    let match_task = uuid_sql::match_uuid_clause("wl.task_id");

    let sql = format!(
        "SELECT
            {} ,
            {} ,
            {} ,
            {} ,
            u.name AS user_name,
            {} ,
            rr.name AS resource_role_name,
            wl.hours,
            wl.hourly_rate_snapshot,
            wl.currency_snapshot,
            wl.cost_amount,
            wl.work_date,
            wl.note,
            wl.source,
            wl.created_at,
            wl.updated_at,
            wl.deleted_at
         FROM work_logs wl
         LEFT JOIN users u ON u.id = wl.user_id
         INNER JOIN resource_roles rr ON rr.id = wl.resource_role_id
         WHERE {} AND {} AND {} AND wl.deleted_at IS NULL
         LIMIT 1",
        id_case, project_case, task_case, user_case, role_case, match_id, match_project, match_task
    );

    let row = sqlx::query(&sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("work log not found"))?;

    parse_work_log_row(&row)
}

fn parse_work_log_row(row: &sqlx::sqlite::SqliteRow) -> AppResult<StoredWorkLog> {
    let id: String = row.try_get("id")?;
    let project_id: String = row.try_get("project_id")?;
    let task_id: String = row.try_get("task_id")?;
    let user_id: Option<String> = row.try_get("user_id")?;
    let role_id: String = row.try_get("resource_role_id")?;
    let source: String = row.try_get("source")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;
    let deleted_at: Option<String> = row.try_get("deleted_at")?;

    let source = match source.as_str() {
        "manual" => WorkLogSource::Manual,
        "migrated_task_progress" => WorkLogSource::MigratedTaskProgress,
        "auto_progress" => WorkLogSource::AutoProgress,
        other => {
            return Err(AppError::internal(format!(
                "invalid work_log source: {}",
                other
            )));
        }
    };

    Ok(StoredWorkLog {
        id: Uuid::parse_str(&id)
            .map_err(|e| AppError::internal(format!("invalid work log id: {}", e)))?,
        project_id: Uuid::parse_str(&project_id)
            .map_err(|e| AppError::internal(format!("invalid project id: {}", e)))?,
        task_id: Uuid::parse_str(&task_id)
            .map_err(|e| AppError::internal(format!("invalid task id: {}", e)))?,
        user_id: user_id
            .map(|v| {
                Uuid::parse_str(&v)
                    .map_err(|e| AppError::internal(format!("invalid user id: {}", e)))
            })
            .transpose()?,
        user_name: row.try_get("user_name")?,
        resource_role_id: Uuid::parse_str(&role_id)
            .map_err(|e| AppError::internal(format!("invalid resource role id: {}", e)))?,
        resource_role_name: row.try_get("resource_role_name")?,
        hours: row.try_get("hours")?,
        hourly_rate_snapshot: row.try_get("hourly_rate_snapshot")?,
        currency_snapshot: row.try_get("currency_snapshot")?,
        cost_amount: row.try_get("cost_amount")?,
        work_date: row.try_get("work_date")?,
        note: row.try_get("note")?,
        source,
        created_at: parse_db_datetime(&created_at)?,
        updated_at: parse_db_datetime(&updated_at)?,
        deleted_at: deleted_at.map(|v| parse_db_datetime(&v)).transpose()?,
    })
}

// ---------------------------------------------------------------------------
// Auto-progress cost logging
// ---------------------------------------------------------------------------

/// Automatically create `auto_progress` work log entries when a task's progress
/// moves forward and the task has enough data to calculate a cost estimate.
///
/// Silently skips (returns `Ok(())`) when:
/// - `progress_delta <= 0`  (backwards or no change — cost never goes back)
/// - `duration_days` is `None`  (no timeline set)
/// - `assignee_id` is `None`  (unassigned task)
/// - The assignee has no active project membership
/// - The assignee has no resource roles assigned
///
/// Hours per day defaults to `WORKING_HOURS_PER_DAY` env var, falling back to `8`.
/// When the assignee holds multiple resource roles the hours are split equally
/// and one work log entry is created per role.
pub async fn auto_log_progress_cost(
    pool: &crate::db::DbPool,
    project_id: Uuid,
    task_id: Uuid,
    assignee_id: Option<Uuid>,
    old_progress: i32,
    new_progress: i32,
    duration_days: Option<i32>,
    actor_id: Uuid,
    now: chrono::DateTime<chrono::Utc>,
) -> AppResult<()> {
    let progress_delta = new_progress - old_progress;
    if progress_delta <= 0 {
        return Ok(());
    }
    let Some(days) = duration_days else {
        return Ok(());
    };
    let Some(assignee) = assignee_id else {
        return Ok(());
    };

    let hours_per_day: f64 = std::env::var("WORKING_HOURS_PER_DAY")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&h: &f64| h > 0.0)
        .unwrap_or(8.0);

    let hours_for_task = days as f64 * hours_per_day;
    let hours_to_log = (progress_delta as f64 / 100.0) * hours_for_task;
    if hours_to_log <= 0.0 {
        return Ok(());
    }

    // Find active project membership for the assignee (silent skip if not a member)
    let match_project = uuid_sql::match_uuid_clause("project_id");
    let match_user = uuid_sql::match_uuid_clause("user_id");
    let id_case = uuid_sql::case_uuid("id");
    let membership_sql = format!(
        "SELECT {} FROM project_members WHERE {} AND {} AND deleted_at IS NULL LIMIT 1",
        id_case, match_project, match_user
    );
    let membership_str: Option<String> = sqlx::query_scalar(&membership_sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(assignee.to_string())
        .bind(assignee.to_string())
        .fetch_optional(pool)
        .await?;
    let Some(membership_str) = membership_str else {
        return Ok(());
    };
    let membership_id = Uuid::parse_str(&membership_str)
        .map_err(|e| AppError::internal(format!("invalid membership id: {}", e)))?;

    // Fetch all active resource roles for this membership, ordered by name for determinism
    let match_membership = uuid_sql::match_uuid_clause("pmrr.membership_id");
    let role_id_case = uuid_sql::case_uuid("rr.id");
    let roles_sql = format!(
        "SELECT {}, rr.name
         FROM project_member_resource_roles pmrr
         JOIN resource_roles rr ON rr.id = pmrr.resource_role_id
         WHERE {} AND pmrr.deleted_at IS NULL AND rr.deleted_at IS NULL
         ORDER BY rr.name ASC",
        role_id_case, match_membership
    );
    let role_rows = sqlx::query(&roles_sql)
        .bind(membership_id.to_string())
        .bind(membership_id.to_string())
        .fetch_all(pool)
        .await?;

    if role_rows.is_empty() {
        return Ok(());
    }

    let hours_per_role = round2(hours_to_log / role_rows.len() as f64);
    let work_date = now.format("%Y-%m-%d").to_string();
    let note = format!("Auto: {}% → {}% progress", old_progress, new_progress);

    for row in &role_rows {
        let role_id_str: String = row.try_get("id")?;
        let role_id = Uuid::parse_str(&role_id_str)
            .map_err(|e| AppError::internal(format!("invalid resource role id: {}", e)))?;

        // Resolve hourly rate: project override → resource role default
        let (hourly_rate, currency) = {
            let match_proj = uuid_sql::match_uuid_clause("prr.project_id");
            let match_role = uuid_sql::match_uuid_clause("prr.resource_role_id");
            let rate_sql = format!(
                "SELECT prr.hourly_rate, prr.currency
                 FROM project_resource_role_rates prr
                 WHERE {} AND {} AND prr.deleted_at IS NULL
                 LIMIT 1",
                match_proj, match_role
            );
            if let Some(rate_row) = sqlx::query(&rate_sql)
                .bind(project_id.to_string())
                .bind(project_id.to_string())
                .bind(role_id.to_string())
                .bind(role_id.to_string())
                .fetch_optional(pool)
                .await?
            {
                let rate: f64 = rate_row.try_get("hourly_rate")?;
                let cur: String = rate_row.try_get("currency")?;
                (rate, cur)
            } else {
                let match_role_id = uuid_sql::match_uuid_clause("id");
                let default_sql = format!(
                    "SELECT default_hourly_rate, currency FROM resource_roles
                     WHERE {} AND deleted_at IS NULL LIMIT 1",
                    match_role_id
                );
                let default_row = sqlx::query(&default_sql)
                    .bind(role_id.to_string())
                    .bind(role_id.to_string())
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::internal("resource role vanished during auto-log"))?;
                let rate: f64 = default_row.try_get("default_hourly_rate")?;
                let cur: String = default_row.try_get("currency")?;
                (rate, cur)
            }
        };

        let cost_amount = round2(hours_per_role * hourly_rate);

        // Use the subselect pattern to store the canonical DB-format UUID for FK columns
        let project_match = uuid_sql::match_uuid_clause("id");
        let task_match = uuid_sql::match_uuid_clause("id");
        let user_match = uuid_sql::match_uuid_clause("id");
        let role_match = uuid_sql::match_uuid_clause("id");
        let actor_match = uuid_sql::match_uuid_clause("id");
        let insert_sql = format!(
            "INSERT INTO work_logs (
                id, project_id, task_id, user_id, resource_role_id,
                hours, hourly_rate_snapshot, currency_snapshot, cost_amount,
                work_date, note, source, created_at, created_by, updated_at, updated_by
             ) VALUES (
                ?,
                (SELECT id FROM projects WHERE {} AND deleted_at IS NULL),
                (SELECT id FROM tasks   WHERE {} AND deleted_at IS NULL),
                (SELECT id FROM users   WHERE {} AND deleted_at IS NULL),
                (SELECT id FROM resource_roles WHERE {} AND deleted_at IS NULL),
                ?, ?, ?, ?, ?, ?, 'auto_progress', ?,
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
                ?,
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
             )",
            project_match, task_match, user_match, role_match, actor_match, actor_match
        );

        sqlx::query(&insert_sql)
            .bind(Uuid::new_v4().to_string())
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .bind(task_id.to_string())
            .bind(task_id.to_string())
            .bind(assignee.to_string())
            .bind(assignee.to_string())
            .bind(role_id.to_string())
            .bind(role_id.to_string())
            .bind(hours_per_role)
            .bind(hourly_rate)
            .bind(&currency)
            .bind(cost_amount)
            .bind(&work_date)
            .bind(&note)
            .bind(now)
            .bind(actor_id.to_string())
            .bind(actor_id.to_string())
            .bind(now)
            .bind(actor_id.to_string())
            .bind(actor_id.to_string())
            .execute(pool)
            .await?;
    }

    Ok(())
}
