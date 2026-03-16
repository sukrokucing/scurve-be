use std::collections::HashSet;

use crate::db::{row_parsers, uuid_sql};
use axum::extract::{Path, Query, State};
use axum::http::header::HeaderName;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::dependency::{DependencyCreateRequest, TaskDependency};
use crate::models::task::{
    managed_completed_at, managed_completed_at_is_backfilled, DbTask, Task, TaskActivityEntry,
    TaskAssignee, TaskBatchDeleteRequest, TaskBatchDeleteResponse, TaskCreateRequest,
    TaskHealthStatus, TaskProgressMethod, TaskScheduleStatus, TaskUpdateRequest,
};
use crate::models::task_progress_component::{
    ReplaceTaskProgressComponentsRequest, TaskProgressComponent, TaskProgressComponentInput,
};
use crate::task_metrics;
use crate::utils::{normalize_to_midnight, utc_now};

#[derive(Debug, Deserialize, Clone, Copy, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskSortBy {
    StartDate,
    DueDate,
    CreatedAt,
    UpdatedAt,
    Title,
    Status,
    Progress,
    ExpectedProgressPct,
    ActualProgressPct,
    VariancePct,
    HealthStatus,
}

#[derive(Debug, Deserialize, Clone, Copy, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskSortDir {
    Asc,
    Desc,
}

#[derive(Debug, Deserialize, Default)]
pub struct TaskListQuery {
    pub progress: Option<bool>,
    pub task_id: Option<Uuid>,
    pub q: Option<String>,
    pub status: Option<String>,
    pub schedule_status: Option<String>,
    pub health_status: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub start_from: Option<String>,
    pub start_to: Option<String>,
    pub due_from: Option<String>,
    pub due_to: Option<String>,
    pub sort_by: Option<TaskSortBy>,
    pub sort_dir: Option<TaskSortDir>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks",
    tag = "Tasks",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("progress" = Option<bool>, Query, description = "Legacy compatibility flag. When true, task list response is empty; use /progress endpoints instead."),
        ("task_id" = Option<Uuid>, Query, description = "Optional task filter used only with progress=true."),
        ("q" = Option<String>, Query, description = "Title search keyword. Trimmed; max 128 characters. '%' and '_' are treated as literal characters."),
        ("status" = Option<String>, Query, description = "Filter by task status. Supports comma-separated values (e.g. todo,done)."),
        ("schedule_status" = Option<String>, Query, description = "Filter by backend-computed schedule status. Supports comma-separated values: finished_early, overdue, on_time, not_specified."),
        ("health_status" = Option<String>, Query, description = "Filter by derived task health status. Supports comma-separated values: ahead, on_track, at_risk, critical, needs_plan."),
        ("assignee_id" = Option<Uuid>, Query, description = "Filter by assignee user id."),
        ("start_from" = Option<String>, Query, description = "Filter tasks with start_date >= this timestamp (RFC3339 or YYYY-MM-DD)."),
        ("start_to" = Option<String>, Query, description = "Filter tasks with start_date <= this timestamp (RFC3339 or YYYY-MM-DD)."),
        ("due_from" = Option<String>, Query, description = "Filter tasks with due_date >= this timestamp (RFC3339 or YYYY-MM-DD)."),
        ("due_to" = Option<String>, Query, description = "Filter tasks with due_date <= this timestamp (RFC3339 or YYYY-MM-DD)."),
        ("sort_by" = Option<TaskSortBy>, Query, description = "Sort field."),
        ("sort_dir" = Option<TaskSortDir>, Query, description = "Sort direction."),
        ("page" = Option<u32>, Query, description = "Page number (1-based, default 1). Values below 1 are treated as 1."),
        ("per_page" = Option<u32>, Query, description = "Items per page (default 50). Clamped to 1..100.")
    ),
    responses((status = 200, description = "List tasks", body = [Task], headers(
        ("X-Total-Count" = i64, description = "Total number of matching tasks")
    ))),
    security(("bearerAuth" = []))
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TaskListQuery>,
    auth: AuthUser,
) -> AppResult<(HeaderMap, Json<Vec<Task>>)> {
    // If caller requested progress via query param, return progress entries instead
    if query.progress.unwrap_or(false) {
        // verify project membership
        ensure_project_membership(&state.pool, auth.user_id, project_id).await?;
        if let Some(task_id) = query.task_id {
            // Keep legacy behavior: validate the referenced task belongs to the project.
            let _ = fetch_task(&state.pool, auth.user_id, project_id, task_id).await?;
        }

        // Convert to Progress and then to Task-like JSON via serde Value? We will return empty Vec<Task> to satisfy signature
        // But to avoid breaking the signature, we'll return an empty task list when progress=true — caller should use the progress endpoints.
        // For now, return an empty Vec<Task> as placeholder.
        let tasks: Vec<Task> = Vec::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-total-count"),
            "0".parse().unwrap(),
        );
        return Ok((headers, Json(tasks)));
    }

    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let offset = ((page - 1) * per_page) as usize;
    let search_q = normalize_search_query(query.q)?;
    let schedule_statuses = normalize_schedule_status_filter(query.schedule_status)?;
    let health_statuses = normalize_health_status_filter(query.health_status)?;
    let sort_by = query.sort_by.unwrap_or(TaskSortBy::StartDate);
    let sort_dir = query.sort_dir.unwrap_or(TaskSortDir::Asc);

    let match_proj = uuid_sql::match_uuid_clause("t.project_id");
    let mut conditions = vec![match_proj, "t.deleted_at IS NULL".to_string()];
    let mut binds: Vec<String> = vec![project_id.to_string(), project_id.to_string()];

    if let Some(q) = search_q {
        conditions.push("LOWER(t.title) LIKE LOWER(?) ESCAPE '\\'".to_string());
        binds.push(format!("%{}%", escape_like_pattern(&q)));
    }

    if let Some(status_raw) = query.status {
        let statuses = split_csv_values(&status_raw);
        if statuses.is_empty() {
            return Err(AppError::bad_request("status must not be empty"));
        }
        if statuses.len() == 1 {
            conditions.push("LOWER(t.status) = LOWER(?)".to_string());
            binds.push(statuses[0].to_string());
        } else {
            let placeholders = std::iter::repeat_n("LOWER(?)", statuses.len())
                .collect::<Vec<_>>()
                .join(", ");
            conditions.push(format!("LOWER(t.status) IN ({})", placeholders));
            for status in statuses {
                binds.push(status.to_string());
            }
        }
    }

    if let Some(assignee_id) = query.assignee_id {
        conditions.push(format!("({})", uuid_sql::match_uuid_clause("t.assignee")));
        binds.push(assignee_id.to_string());
        binds.push(assignee_id.to_string());
    }

    if let Some(start_from) = query.start_from {
        let start_from = parse_filter_datetime("start_from", &start_from, DateBound::From)?;
        conditions
            .push("t.start_date IS NOT NULL AND datetime(t.start_date) >= datetime(?)".to_string());
        binds.push(start_from);
    }

    if let Some(start_to) = query.start_to {
        let start_to = parse_filter_datetime("start_to", &start_to, DateBound::To)?;
        conditions
            .push("t.start_date IS NOT NULL AND datetime(t.start_date) <= datetime(?)".to_string());
        binds.push(start_to);
    }

    if let Some(due_from) = query.due_from {
        let due_from = parse_filter_datetime("due_from", &due_from, DateBound::From)?;
        conditions
            .push("t.due_date IS NOT NULL AND datetime(t.due_date) >= datetime(?)".to_string());
        binds.push(due_from);
    }

    if let Some(due_to) = query.due_to {
        let due_to = parse_filter_datetime("due_to", &due_to, DateBound::To)?;
        conditions
            .push("t.due_date IS NOT NULL AND datetime(t.due_date) <= datetime(?)".to_string());
        binds.push(due_to);
    }

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT {} \
         FROM tasks t \
         WHERE {}",
        task_metrics::task_select_columns("t"),
        where_clause
    );

    let mut query_exec = sqlx::query(&sql);
    for bind in &binds {
        query_exec = query_exec.bind(bind);
    }

    let rows = query_exec.fetch_all(&state.pool).await?;

    let mut tasks_rows = Vec::with_capacity(rows.len());
    for row in rows {
        tasks_rows.push(row_parsers::db_task_from_row(&row)?);
    }

    let mut tasks =
        task_metrics::build_task_responses(&state.pool, project_id, tasks_rows, utc_now()).await?;

    if let Some(schedule_statuses) = schedule_statuses {
        tasks.retain(|task| schedule_statuses.contains(&task.schedule_status));
    }

    if let Some(health_statuses) = health_statuses {
        tasks.retain(|task| health_statuses.contains(&task.health_status));
    }

    sort_tasks(&mut tasks, sort_by, sort_dir);
    let total_count = tasks.len();
    let tasks = tasks
        .into_iter()
        .skip(offset)
        .take(per_page as usize)
        .collect::<Vec<_>>();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-total-count"),
        total_count.to_string().parse().unwrap(),
    );

    Ok((headers, Json(tasks)))
}

#[utoipa::path(
    post,
    path = "/projects/{project_id}/tasks",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = TaskCreateRequest,
    responses((status = 201, description = "Task created", body = Task)),
    security(("bearerAuth" = []))
)]
pub async fn create_task(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Json(payload): Json<TaskCreateRequest>,
) -> AppResult<(StatusCode, Json<Task>)> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let task_id = Uuid::new_v4();
    let now = utc_now();
    let status = payload
        .status
        .clone()
        .unwrap_or_else(|| "pending".to_string());
    let description = normalize_create_description(&payload.title, payload.description.as_deref());
    let progress_method = payload.progress_method.unwrap_or_default();
    let blocked_flag = payload.blocked_flag.unwrap_or(false);
    let blocked_reason = normalize_optional_text(payload.blocked_reason.as_deref());
    let start_date = payload.start_date;
    let end_date = payload.end_date;
    let baseline_start_at = payload.baseline_start_at;
    let baseline_end_at = payload.baseline_end_at;
    let task_weight = validate_task_weight(payload.task_weight.unwrap_or(1.0))?;
    validate_progress_payload(progress_method, payload.progress)?;
    let progress = payload.progress.unwrap_or(0);
    let completed_at = managed_completed_at(&status, progress, None, now);
    let completed_at_is_backfilled = false;

    if let (Some(start), Some(end)) = (start_date, end_date) {
        if end < start {
            return Err(AppError::bad_request("end_date must be >= start_date"));
        }
    }
    validate_baseline(baseline_start_at, baseline_end_at)?;

    if let Some(p) = payload.progress {
        if !(0..=100).contains(&p) {
            return Err(AppError::bad_request("progress must be between 0 and 100"));
        }
    }

    let match_proj = uuid_sql::match_uuid_clause("id");
    let insert_sql = format!(
        "INSERT INTO tasks (id, project_id, title, description, status, progress_method, blocked_flag, blocked_reason, due_date, start_date, end_date, baseline_start_at, baseline_end_at, task_weight, assignee, parent_id, progress, completed_at, completed_at_is_backfilled, created_at, updated_at) \
         VALUES (?, (SELECT id FROM projects WHERE {} AND deleted_at IS NULL), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        match_proj
    );

    sqlx::query(&insert_sql)
        .bind(task_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(&payload.title)
        .bind(description)
        .bind(status)
        .bind(progress_method.as_str())
        .bind(blocked_flag)
        .bind(blocked_reason)
        .bind(payload.due_date)
        .bind(start_date)
        .bind(end_date)
        .bind(baseline_start_at)
        .bind(baseline_end_at)
        .bind(task_weight)
        .bind(payload.assignee.map(|id| id.to_string()))
        .bind(payload.parent_id.map(|id| id.to_string()))
        .bind(progress)
        .bind(completed_at)
        .bind(completed_at_is_backfilled)
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await?;

    if progress_method == TaskProgressMethod::WeightedComponents {
        task_metrics::refresh_task_snapshot(&state.pool, task_id).await?;
    }
    let task = fetch_task(&state.pool, auth.user_id, project_id, task_id).await?;
    let task_dto =
        task_metrics::build_task_response(&state.pool, project_id, task.clone(), now).await?;

    // Log activity with request context (no old state for create)
    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "created",
        Some(auth.user_id),
        &task_dto,
        None,
        Some(ctx),
    );

    Ok((StatusCode::CREATED, Json(task_dto)))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/{id}",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id"), ("id" = Uuid, Path, description = "Task id")),
    request_body = TaskUpdateRequest,
    responses((status = 200, description = "Task updated", body = Task)),
    security(("bearerAuth" = []))
)]
pub async fn update_task(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<TaskUpdateRequest>,
) -> AppResult<Json<Task>> {
    // Capture old state BEFORE modifications
    let old_task = fetch_task(&state.pool, auth.user_id, project_id, id).await?;
    let old_dto =
        task_metrics::build_task_response(&state.pool, project_id, old_task.clone(), utc_now())
            .await?;

    let mut task = old_task;

    let TaskUpdateRequest {
        title,
        description,
        status,
        progress_method,
        blocked_flag,
        blocked_reason,
        due_date,
        start_date,
        end_date,
        baseline_start_at,
        baseline_end_at,
        task_weight,
        assignee,
        parent_id,
        progress,
    } = payload;

    if let Some(title) = title {
        task.title = title;
    }
    if let Some(description) = description {
        if description.trim().is_empty() {
            return Err(AppError::bad_request("description must not be empty"));
        }
        task.description = description;
    }
    if let Some(status) = status {
        task.status = status;
    }
    if let Some(progress_method) = progress_method {
        task.progress_method = progress_method;
    }
    if let Some(blocked_flag) = blocked_flag {
        task.blocked_flag = blocked_flag;
    }
    if blocked_reason.is_some() {
        task.blocked_reason = normalize_optional_text(blocked_reason.as_deref());
    }
    if let Some(due_date) = due_date {
        task.due_date = Some(due_date);
    }

    if let Some(sd) = start_date {
        task.start_date = Some(sd);
    }
    if let Some(ed) = end_date {
        task.end_date = Some(ed);
    }
    if let Some(baseline_start_at) = baseline_start_at {
        task.baseline_start_at = Some(baseline_start_at);
    }
    if let Some(baseline_end_at) = baseline_end_at {
        task.baseline_end_at = Some(baseline_end_at);
    }
    if let Some(task_weight) = task_weight {
        task.task_weight = validate_task_weight(task_weight)?;
    }
    if let Some(a) = assignee {
        task.assignee = Some(a);
    }
    if let Some(pid) = parent_id {
        task.parent_id = Some(pid);
    }
    if let Some(p) = progress {
        validate_progress_payload(task.progress_method, Some(p))?;
        if !(0..=100).contains(&p) {
            return Err(AppError::bad_request("progress must be between 0 and 100"));
        }
        task.progress = p;
    }

    // Validate timeline fields if both are present
    if let (Some(sd), Some(ed)) = (task.start_date, task.end_date) {
        if ed < sd {
            return Err(AppError::bad_request("end_date must be >= start_date"));
        }
    }
    validate_baseline(task.baseline_start_at, task.baseline_end_at)?;

    let now = utc_now();
    let next_completed_at =
        managed_completed_at(&task.status, task.progress, task.completed_at, now);
    task.completed_at_is_backfilled = managed_completed_at_is_backfilled(
        task.completed_at,
        task.completed_at_is_backfilled,
        next_completed_at,
    );
    task.completed_at = next_completed_at;

    sqlx::query(
        "UPDATE tasks SET title = ?, description = ?, status = ?, progress_method = ?, blocked_flag = ?, blocked_reason = ?, due_date = ?, start_date = ?, end_date = ?, baseline_start_at = ?, baseline_end_at = ?, task_weight = ?, assignee = ?, parent_id = ?, progress = ?, completed_at = ?, completed_at_is_backfilled = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.status)
    .bind(task.progress_method.as_str())
    .bind(task.blocked_flag)
    .bind(&task.blocked_reason)
    .bind(task.due_date)
    .bind(task.start_date)
    .bind(task.end_date)
    .bind(task.baseline_start_at)
    .bind(task.baseline_end_at)
    .bind(task.task_weight)
    .bind(task.assignee.map(|id| id.to_string()))
    .bind(task.parent_id.map(|id| id.to_string()))
    .bind(task.progress)
    .bind(task.completed_at)
    .bind(task.completed_at_is_backfilled)
    .bind(now)
    .bind(task.id.to_string())
    .execute(&state.pool)
    .await?;

    if task.progress_method == TaskProgressMethod::WeightedComponents {
        task_metrics::refresh_task_snapshot(&state.pool, task.id).await?;
    }
    // Re-fetch to get the DB-calculated fields (like duration_days from triggers)
    let task = fetch_task(&state.pool, auth.user_id, project_id, task.id).await?;
    let task_dto =
        task_metrics::build_task_response(&state.pool, project_id, task.clone(), now).await?;

    // Log activity with old/new tracking and request context
    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "updated",
        Some(auth.user_id),
        &task_dto,
        Some(&old_dto),
        Some(ctx),
    );

    Ok(Json(task_dto))
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{id}",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id"), ("id" = Uuid, Path, description = "Task id")),
    responses((status = 200, description = "Task detail", body = Task)),
    security(("bearerAuth" = []))
)]
pub async fn get_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Task>> {
    let task = fetch_task(&state.pool, auth.user_id, project_id, id).await?;
    let task = task_metrics::build_task_response(&state.pool, project_id, task, utc_now()).await?;
    Ok(Json(task))
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{id}/progress-components",
    tag = "Tasks",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Task id")
    ),
    responses((status = 200, description = "List task progress components", body = [TaskProgressComponent])),
    security(("bearerAuth" = []))
)]
pub async fn list_task_progress_components(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<TaskProgressComponent>>> {
    let _ = fetch_task(&state.pool, auth.user_id, project_id, id).await?;
    let components = task_metrics::load_task_progress_components(&state.pool, id).await?;
    Ok(Json(components))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/{id}/progress-components",
    tag = "Tasks",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Task id")
    ),
    request_body = ReplaceTaskProgressComponentsRequest,
    responses((status = 200, description = "Replace task progress components", body = [TaskProgressComponent])),
    security(("bearerAuth" = []))
)]
pub async fn replace_task_progress_components(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<ReplaceTaskProgressComponentsRequest>,
) -> AppResult<Json<Vec<TaskProgressComponent>>> {
    let task = fetch_task(&state.pool, auth.user_id, project_id, id).await?;
    if task.progress_method != TaskProgressMethod::WeightedComponents {
        return Err(AppError::bad_request(
            "progress-components require progress_method=weighted_components",
        ));
    }

    validate_component_payloads(&payload.components)?;

    let mut tx = state.pool.begin().await?;
    let now = utc_now();

    let existing_ids = sqlx::query(&format!(
        "SELECT {} FROM task_progress_components WHERE {} AND deleted_at IS NULL",
        uuid_sql::case_uuid("id"),
        uuid_sql::match_uuid_clause("task_id")
    ))
    .bind(id.to_string())
    .bind(id.to_string())
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|row| row.try_get::<String, _>("id"))
    .collect::<Result<HashSet<_>, _>>()?;

    let mut keep_ids = HashSet::new();
    for (index, component) in payload.components.iter().enumerate() {
        let component_id = component.id.unwrap_or_else(Uuid::new_v4);
        let sort_order = component.sort_order.unwrap_or(index as i32);
        keep_ids.insert(component_id.to_string());

        if existing_ids.contains(&component_id.to_string()) {
            let sql = format!(
                "UPDATE task_progress_components
                 SET name = ?, component_type = ?, weight = ?, completion_pct = ?, planned_at = ?, completed_at = ?, sort_order = ?, updated_at = ?, deleted_at = NULL
                 WHERE {} AND {}",
                uuid_sql::match_uuid_clause("id"),
                uuid_sql::match_uuid_clause("task_id")
            );
            sqlx::query(&sql)
                .bind(component.name.trim())
                .bind(component.component_type.trim())
                .bind(component.weight)
                .bind(component.completion_pct)
                .bind(component.planned_at)
                .bind(component.completed_at)
                .bind(sort_order)
                .bind(now)
                .bind(component_id.to_string())
                .bind(component_id.to_string())
                .bind(id.to_string())
                .bind(id.to_string())
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "INSERT INTO task_progress_components (id, task_id, name, component_type, weight, completion_pct, planned_at, completed_at, sort_order, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(component_id.to_string())
            .bind(id.to_string())
            .bind(component.name.trim())
            .bind(component.component_type.trim())
            .bind(component.weight)
            .bind(component.completion_pct)
            .bind(component.planned_at)
            .bind(component.completed_at)
            .bind(sort_order)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
    }

    let rows = sqlx::query(&format!(
        "SELECT {} FROM task_progress_components WHERE {} AND deleted_at IS NULL",
        uuid_sql::case_uuid("id"),
        uuid_sql::match_uuid_clause("task_id")
    ))
    .bind(id.to_string())
    .bind(id.to_string())
    .fetch_all(&mut *tx)
    .await?;

    for row in rows {
        let existing_id: String = row.try_get("id")?;
        if !keep_ids.contains(&existing_id) {
            let sql = format!(
                "UPDATE task_progress_components SET deleted_at = ?, updated_at = ? WHERE {} AND {}",
                uuid_sql::match_uuid_clause("id"),
                uuid_sql::match_uuid_clause("task_id")
            );
            let existing_id_second = existing_id.clone();
            sqlx::query(&sql)
                .bind(now)
                .bind(now)
                .bind(existing_id)
                .bind(existing_id_second)
                .bind(id.to_string())
                .bind(id.to_string())
                .execute(&mut *tx)
                .await?;
        }
    }

    let task_touch_sql = format!(
        "UPDATE tasks SET updated_at = ? WHERE {} AND deleted_at IS NULL",
        uuid_sql::match_uuid_clause("id")
    );
    sqlx::query(&task_touch_sql)
        .bind(now)
        .bind(id.to_string())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    task_metrics::refresh_task_snapshot(&state.pool, id).await?;
    let components = task_metrics::load_task_progress_components(&state.pool, id).await?;
    Ok(Json(components))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/tasks/{id}",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id"), ("id" = Uuid, Path, description = "Task id")),
    responses((status = 204, description = "Task soft deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let _ = fetch_task(&state.pool, auth.user_id, project_id, id).await?;

    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "UPDATE tasks SET deleted_at = ?, updated_at = ? WHERE {} AND {} AND deleted_at IS NULL",
        match_id, match_proj
    );

    let affected = sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("task not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/assignees",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List assignees in project tasks", body = [TaskAssignee])),
    security(("bearerAuth" = []))
)]
pub async fn list_project_assignees(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    auth: AuthUser,
) -> AppResult<Json<Vec<TaskAssignee>>> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let assignee_case = uuid_sql::case_uuid("t.assignee");
    let user_id_case = uuid_sql::case_uuid("u.id");
    let match_proj = uuid_sql::match_uuid_clause("t.project_id");
    let sql = format!(
        "SELECT DISTINCT u.id, u.name, u.email
         FROM (
            SELECT DISTINCT {} FROM tasks t
            WHERE {} AND t.assignee IS NOT NULL AND t.deleted_at IS NULL
         ) ta
         INNER JOIN (
            SELECT {}, name, email, deleted_at FROM users u
         ) u ON u.id = ta.assignee
         WHERE u.deleted_at IS NULL
         ORDER BY u.name ASC",
        assignee_case, match_proj, user_id_case
    );

    let rows = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut assignees = Vec::with_capacity(rows.len());
    for row in rows {
        let id_s: String = row.try_get("id")?;
        let id = Uuid::parse_str(&id_s)
            .map_err(|e| AppError::internal(format!("invalid assignee uuid: {}", e)))?;
        let name: String = row.try_get("name")?;
        let email: String = row.try_get("email")?;
        assignees.push(TaskAssignee { id, name, email });
    }

    Ok(Json(assignees))
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{id}/activity",
    tag = "Tasks",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Task id")
    ),
    responses((status = 200, description = "Task activity timeline", body = [TaskActivityEntry])),
    security(("bearerAuth" = []))
)]
pub async fn list_task_activity(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<TaskActivityEntry>>> {
    let _ = fetch_task(&state.pool, auth.user_id, project_id, id).await?;

    let match_subject = uuid_sql::match_uuid_clause("subject_id");
    let sql = format!(
        "SELECT id, event_name, actor_id, properties, occurred_at
         FROM activity_log
         WHERE {} AND event_name LIKE 'task.%'
         ORDER BY occurred_at DESC",
        match_subject
    );

    let rows = sqlx::query(&sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let event_id: String = row.try_get("id").unwrap_or_default();
        let action: String = row.try_get("event_name").unwrap_or_default();
        let actor_id = row
            .try_get::<Option<String>, _>("actor_id")
            .ok()
            .flatten()
            .and_then(|s| Uuid::parse_str(&s).ok());
        let occurred_at: chrono::DateTime<chrono::Utc> = row
            .try_get("occurred_at")
            .unwrap_or_else(|_| chrono::Utc::now());
        let details = row
            .try_get::<Option<String>, _>("properties")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or(serde_json::Value::Null);

        items.push(TaskActivityEntry {
            id: event_id,
            action,
            actor_id,
            occurred_at,
            details,
        });
    }

    Ok(Json(items))
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/dependencies",
    tag = "Dependencies",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List dependencies", body = [TaskDependency])),
    security(("bearerAuth" = []))
)]
pub async fn list_dependencies(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    auth: AuthUser,
) -> AppResult<Json<Vec<TaskDependency>>> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    // Use a defensive manual SELECT that textifies UUIDs and parses rows explicitly.
    let id_case = uuid_sql::case_uuid("d.id");
    let source_case = uuid_sql::case_uuid("d.source_task_id");
    let target_case = uuid_sql::case_uuid("d.target_task_id");
    let project_match = uuid_sql::match_uuid_clause("t.project_id");
    let sql = format!(
        "SELECT {} , {} , {} , d.type, d.created_at FROM task_dependencies d INNER JOIN tasks t ON t.id = d.source_task_id WHERE {} AND t.deleted_at IS NULL",
        id_case, source_case, target_case, project_match
    );

    let rows = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        parsed.push(row_parsers::db_task_dependency_from_row(&row)?);
    }

    let deps_rows = parsed;

    let deps: Vec<TaskDependency> = deps_rows
        .into_iter()
        .map(TaskDependency::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(deps))
}

#[utoipa::path(
    post,
    path = "/projects/{project_id}/dependencies",
    tag = "Dependencies",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = DependencyCreateRequest,
    responses((status = 201, description = "Dependency created", body = TaskDependency)),
    security(("bearerAuth" = []))
)]
pub async fn create_dependency(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    auth: AuthUser,
    Json(payload): Json<DependencyCreateRequest>,
) -> AppResult<(StatusCode, Json<TaskDependency>)> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    // Validate tasks exist and belong to project
    let _source = fetch_task(
        &state.pool,
        auth.user_id,
        project_id,
        payload.source_task_id,
    )
    .await?;
    let _target = fetch_task(
        &state.pool,
        auth.user_id,
        project_id,
        payload.target_task_id,
    )
    .await?;

    if payload.source_task_id == payload.target_task_id {
        return Err(AppError::bad_request("Cannot link task to itself"));
    }

    // Check for existing reverse link to prevent immediate cycle (A->B and B->A)
    let rev_source_match = uuid_sql::match_uuid_clause("source_task_id");
    let rev_target_match = uuid_sql::match_uuid_clause("target_task_id");
    let reverse_sql = format!(
        "SELECT EXISTS(SELECT 1 FROM task_dependencies WHERE {} AND {})",
        rev_source_match, rev_target_match
    );

    let reverse_exists: bool = sqlx::query_scalar(&reverse_sql)
        .bind(payload.target_task_id.to_string())
        .bind(payload.target_task_id.to_string())
        .bind(payload.source_task_id.to_string())
        .bind(payload.source_task_id.to_string())
        .fetch_one(&state.pool)
        .await?;

    if reverse_exists {
        return Err(AppError::bad_request(
            "Cycle detected: reverse dependency already exists",
        ));
    }

    // Detect deeper cycles using a recursive CTE.
    let cycle_source_match = uuid_sql::match_uuid_clause("source_task_id");
    let cycle_reach_match = uuid_sql::match_uuid_clause("node");
    let cycle_sql = format!(
        "WITH RECURSIVE reach(node) AS (
            SELECT target_task_id FROM task_dependencies WHERE {}
            UNION
            SELECT d.target_task_id FROM task_dependencies d JOIN reach r ON d.source_task_id = r.node
        )
        SELECT EXISTS(SELECT 1 FROM reach WHERE {});",
        cycle_source_match, cycle_reach_match
    );

    let cycle_exists: bool = sqlx::query_scalar(&cycle_sql)
        .bind(payload.target_task_id.to_string())
        .bind(payload.target_task_id.to_string())
        .bind(payload.source_task_id.to_string())
        .bind(payload.source_task_id.to_string())
        .fetch_one(&state.pool)
        .await?;

    if cycle_exists {
        return Err(AppError::bad_request(
            "Cycle detected: would create circular dependency",
        ));
    }

    let id = Uuid::new_v4();
    let now = utc_now();

    let match_source = uuid_sql::match_uuid_clause("id");
    let match_target = uuid_sql::match_uuid_clause("id");
    let insert_sql = format!(
        "INSERT INTO task_dependencies (id, source_task_id, target_task_id, type, created_at) \
         VALUES (?, (SELECT id FROM tasks WHERE {}), (SELECT id FROM tasks WHERE {}), ?, ?)",
        match_source, match_target
    );

    sqlx::query(&insert_sql)
        .bind(id.to_string())
        .bind(payload.source_task_id.to_string())
        .bind(payload.source_task_id.to_string())
        .bind(payload.target_task_id.to_string())
        .bind(payload.target_task_id.to_string())
        .bind(&payload.type_)
        .bind(now)
        .execute(&state.pool)
        .await?;

    let dep = TaskDependency {
        id,
        source_task_id: payload.source_task_id,
        target_task_id: payload.target_task_id,
        type_: payload.type_,
        created_at: now,
    };

    Ok((StatusCode::CREATED, Json(dep)))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/dependencies/{id}",
    tag = "Dependencies",
    params(("project_id" = Uuid, Path, description = "Project id"), ("id" = Uuid, Path, description = "Dependency id")),
    responses((status = 204, description = "Dependency deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_dependency(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<StatusCode> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    // We need to verify the dependency belongs to a task in this project
    let match_id = uuid_sql::match_uuid_clause("id");
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let delete_sql = format!(
        "DELETE FROM task_dependencies WHERE {} AND source_task_id IN (SELECT id FROM tasks WHERE {})",
        match_id, match_proj
    );

    let affected = sqlx::query(&delete_sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found(
            "Dependency not found or not in project",
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/tasks/batch",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = TaskBatchDeleteRequest,
    responses((status = 200, description = "Tasks soft deleted", body = TaskBatchDeleteResponse)),
    security(("bearerAuth" = []))
)]
pub async fn batch_delete_tasks(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<TaskBatchDeleteRequest>,
) -> AppResult<Json<TaskBatchDeleteResponse>> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    if payload.ids.is_empty() {
        return Err(AppError::bad_request("ids must not be empty"));
    }

    let mut seen = HashSet::new();
    let ids: Vec<Uuid> = payload
        .ids
        .into_iter()
        .filter(|id| seen.insert(*id))
        .collect();

    let mut tx = state.pool.begin().await?;
    let now = utc_now();
    let mut deleted = 0usize;

    for id in ids {
        let match_id = uuid_sql::match_uuid_clause("id");
        let match_proj = uuid_sql::match_uuid_clause("project_id");
        let sql = format!(
            "UPDATE tasks
             SET deleted_at = ?, updated_at = ?
             WHERE {} AND {} AND deleted_at IS NULL",
            match_id, match_proj
        );

        let affected = sqlx::query(&sql)
            .bind(now)
            .bind(now)
            .bind(id.to_string())
            .bind(id.to_string())
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .execute(&mut *tx)
            .await?;

        if affected.rows_affected() == 0 {
            return Err(AppError::not_found(format!(
                "task {} not found in project",
                id
            )));
        }

        deleted += affected.rows_affected() as usize;
    }

    tx.commit().await?;
    Ok(Json(TaskBatchDeleteResponse { deleted }))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/batch",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = TaskBatchUpdatePayload,
    responses((status = 200, description = "Tasks updated", body = [Task])),
    security(("bearerAuth" = []))
)]
pub async fn batch_update_tasks(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<crate::models::task::TaskBatchUpdatePayload>,
) -> AppResult<Json<Vec<Task>>> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let mut tx = state.pool.begin().await?;
    let now = utc_now();
    let mut updated_ids = Vec::new();

    for update in payload.tasks {
        // Verify task belongs to project
        let match_id = uuid_sql::match_uuid_clause("id");
        let match_proj = uuid_sql::match_uuid_clause("project_id");
        let exists_sql = format!(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE {} AND {} AND deleted_at IS NULL)",
            match_id, match_proj
        );

        let exists: bool = sqlx::query_scalar(&exists_sql)
            .bind(update.id.to_string())
            .bind(update.id.to_string())
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .fetch_one(&mut *tx)
            .await?;

        if !exists {
            return Err(AppError::not_found(format!(
                "Task {} not found in project",
                update.id
            )));
        }

        let match_id = uuid_sql::match_uuid_clause("t.id");

        let sql = format!(
            "SELECT {} FROM tasks t WHERE {}",
            task_metrics::task_select_columns("t"),
            match_id
        );

        let row = sqlx::query(&sql)
            .bind(update.id.to_string())
            .bind(update.id.to_string())
            .fetch_one(&mut *tx)
            .await?;

        let current = row_parsers::db_task_from_row(&row)?;

        // Use original dates (removed normalization)
        let start = update
            .start_date
            .or(current.start_date.map(|d| d.with_timezone(&Utc)));
        let end = update
            .end_date
            .or(current.end_date.map(|d| d.with_timezone(&Utc)));

        if let (Some(s), Some(e)) = (start, end) {
            if e < s {
                return Err(AppError::bad_request(format!(
                    "Task {}: end_date must be >= start_date",
                    update.id
                )));
            }
        }

        if let Some(p) = update.progress {
            if current.progress_method == TaskProgressMethod::WeightedComponents {
                return Err(AppError::bad_request(format!(
                    "Task {}: progress updates require manual_percent_legacy progress_method",
                    update.id
                )));
            }
            if !(0..=100).contains(&p) {
                return Err(AppError::bad_request(format!(
                    "Task {}: progress must be between 0 and 100",
                    update.id
                )));
            }
        }

        let title = update.title.unwrap_or_else(|| current.title.clone());
        let description = current.description.clone();
        let status = update.status.unwrap_or_else(|| current.status.clone());
        let due_date = update
            .due_date
            .or(current.due_date.map(|d| d.with_timezone(&Utc)));
        let start_date = update
            .start_date
            .map(normalize_to_midnight)
            .or(current.start_date.map(|d| d.with_timezone(&Utc)));
        let end_date = update
            .end_date
            .map(normalize_to_midnight)
            .or(current.end_date.map(|d| d.with_timezone(&Utc)));
        let assignee = update.assignee.or(current.assignee);
        let parent_id = update.parent_id.or(current.parent_id);
        let progress = update.progress.unwrap_or(current.progress);
        let completed_at = managed_completed_at(&status, progress, current.completed_at, now);
        let completed_at_is_backfilled = managed_completed_at_is_backfilled(
            current.completed_at,
            current.completed_at_is_backfilled,
            completed_at,
        );

        // Convert Option<Uuid> to Option<String> for binding
        let assignee_str = assignee.map(|u| u.to_string());
        let parent_id_str = parent_id.map(|u| u.to_string());

        let match_id = uuid_sql::match_uuid_clause("id");
        let update_sql = format!(
            "UPDATE tasks SET title = ?, description = ?, status = ?, due_date = ?, start_date = ?, end_date = ?, assignee = ?, parent_id = ?, progress = ?, completed_at = ?, completed_at_is_backfilled = ?, updated_at = ? WHERE {}",
            match_id
        );

        sqlx::query(&update_sql)
            .bind(title)
            .bind(description)
            .bind(status)
            .bind(due_date)
            .bind(start_date)
            .bind(end_date)
            .bind(assignee_str)
            .bind(parent_id_str)
            .bind(progress)
            .bind(completed_at)
            .bind(completed_at_is_backfilled)
            .bind(now)
            .bind(update.id.to_string())
            .bind(update.id.to_string())
            .execute(&mut *tx)
            .await?;

        updated_ids.push(update.id);
    }

    tx.commit().await?;

    if updated_ids.is_empty() {
        return Ok(Json(Vec::new()));
    }

    // Use manual column selection to handle TEXT UUIDs
    let placeholders = std::iter::repeat_n("?", updated_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT {} \
         FROM tasks t \
         WHERE t.id IN ({}) ORDER BY t.start_date ASC",
        task_metrics::task_select_columns("t"),
        placeholders
    );

    let mut query = sqlx::query(&sql);
    for id in updated_ids {
        query = query.bind(id.to_string());
    }

    let rows = query.fetch_all(&state.pool).await?;
    let mut tasks_db = Vec::with_capacity(rows.len());
    for row in rows {
        tasks_db.push(row_parsers::db_task_from_row(&row)?);
    }

    let tasks =
        task_metrics::build_task_responses(&state.pool, project_id, tasks_db, utc_now()).await?;

    Ok(Json(tasks))
}

fn split_csv_values(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect()
}

fn normalize_schedule_status_filter(
    raw: Option<String>,
) -> AppResult<Option<Vec<TaskScheduleStatus>>> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let values = split_csv_values(&raw);
    if values.is_empty() {
        return Err(AppError::bad_request("schedule_status must not be empty"));
    }

    values
        .into_iter()
        .map(|value| {
            value
                .parse::<TaskScheduleStatus>()
                .map_err(AppError::bad_request)
        })
        .collect::<AppResult<Vec<_>>>()
        .map(Some)
}

fn normalize_health_status_filter(raw: Option<String>) -> AppResult<Option<Vec<TaskHealthStatus>>> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let values = split_csv_values(&raw);
    if values.is_empty() {
        return Err(AppError::bad_request("health_status must not be empty"));
    }

    values
        .into_iter()
        .map(|value| {
            value
                .parse::<TaskHealthStatus>()
                .map_err(AppError::bad_request)
        })
        .collect::<AppResult<Vec<_>>>()
        .map(Some)
}

fn normalize_search_query(q: Option<String>) -> AppResult<Option<String>> {
    match q {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > 128 {
                return Err(AppError::bad_request("q must be at most 128 characters"));
            }
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn escape_like_pattern(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn normalize_create_description(title: &str, description: Option<&str>) -> String {
    match description.map(str::trim) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => format!("[Quick Add] {}", title.trim()),
    }
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn validate_task_weight(task_weight: f64) -> AppResult<f64> {
    if task_weight <= 0.0 {
        return Err(AppError::bad_request("task_weight must be greater than 0"));
    }
    Ok(task_weight)
}

fn validate_progress_payload(
    progress_method: TaskProgressMethod,
    progress: Option<i32>,
) -> AppResult<()> {
    if progress_method == TaskProgressMethod::WeightedComponents && progress.is_some() {
        return Err(AppError::bad_request(
            "progress writes require manual_percent_legacy progress_method",
        ));
    }
    Ok(())
}

fn validate_baseline(
    baseline_start_at: Option<DateTime<Utc>>,
    baseline_end_at: Option<DateTime<Utc>>,
) -> AppResult<()> {
    if baseline_start_at.is_some() ^ baseline_end_at.is_some() {
        return Err(AppError::bad_request(
            "baseline_start_at and baseline_end_at must both be provided",
        ));
    }
    if let (Some(start), Some(end)) = (baseline_start_at, baseline_end_at) {
        if end <= start {
            return Err(AppError::bad_request(
                "baseline_end_at must be greater than baseline_start_at",
            ));
        }
    }
    Ok(())
}

fn validate_component_payloads(components: &[TaskProgressComponentInput]) -> AppResult<()> {
    let mut seen_ids = HashSet::new();
    for (index, component) in components.iter().enumerate() {
        if let Some(id) = component.id {
            if !seen_ids.insert(id) {
                return Err(AppError::bad_request(format!(
                    "components[{}].id is duplicated",
                    index
                )));
            }
        }
        if component.name.trim().is_empty() {
            return Err(AppError::bad_request(format!(
                "components[{}].name must not be empty",
                index
            )));
        }
        if component.component_type.trim().is_empty() {
            return Err(AppError::bad_request(format!(
                "components[{}].component_type must not be empty",
                index
            )));
        }
        if component.weight <= 0.0 {
            return Err(AppError::bad_request(format!(
                "components[{}].weight must be greater than 0",
                index
            )));
        }
        if !(0.0..=100.0).contains(&component.completion_pct) {
            return Err(AppError::bad_request(format!(
                "components[{}].completion_pct must be between 0 and 100",
                index
            )));
        }
    }
    Ok(())
}

fn sort_tasks(tasks: &mut [Task], sort_by: TaskSortBy, sort_dir: TaskSortDir) {
    tasks.sort_by(|left, right| compare_tasks(left, right, sort_by, sort_dir));
}

fn compare_tasks(
    left: &Task,
    right: &Task,
    sort_by: TaskSortBy,
    sort_dir: TaskSortDir,
) -> std::cmp::Ordering {
    let ordering = match sort_by {
        TaskSortBy::StartDate => left
            .start_date
            .as_ref()
            .unwrap_or(&left.created_at)
            .cmp(right.start_date.as_ref().unwrap_or(&right.created_at)),
        TaskSortBy::DueDate => left
            .due_date
            .as_ref()
            .unwrap_or(&left.created_at)
            .cmp(right.due_date.as_ref().unwrap_or(&right.created_at)),
        TaskSortBy::CreatedAt => left.created_at.cmp(&right.created_at),
        TaskSortBy::UpdatedAt => left.updated_at.cmp(&right.updated_at),
        TaskSortBy::Title => left.title.cmp(&right.title),
        TaskSortBy::Status => left.status.cmp(&right.status),
        TaskSortBy::Progress => left.progress.cmp(&right.progress),
        TaskSortBy::ExpectedProgressPct => {
            cmp_option_f64(left.expected_progress_pct, right.expected_progress_pct)
        }
        TaskSortBy::ActualProgressPct => {
            cmp_option_f64(left.actual_progress_pct, right.actual_progress_pct)
        }
        TaskSortBy::VariancePct => cmp_option_f64(left.variance_pct, right.variance_pct),
        TaskSortBy::HealthStatus => left
            .health_status
            .rank()
            .cmp(&right.health_status.rank())
            .then_with(|| left.title.cmp(&right.title)),
    };

    match sort_dir {
        TaskSortDir::Asc => ordering,
        TaskSortDir::Desc => ordering.reverse(),
    }
}

fn cmp_option_f64(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left
            .partial_cmp(&right)
            .unwrap_or(std::cmp::Ordering::Equal),
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

#[derive(Copy, Clone)]
enum DateBound {
    From,
    To,
}

fn parse_filter_datetime(field: &str, value: &str, bound: DateBound) -> AppResult<String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Ok(dt.with_timezone(&Utc).to_rfc3339());
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let dt = match bound {
            DateBound::From => date.and_hms_opt(0, 0, 0).expect("valid midnight datetime"),
            DateBound::To => date
                .and_hms_opt(23, 59, 59)
                .expect("valid end-of-day datetime"),
        };
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339());
    }

    Err(AppError::bad_request(format!(
        "{} must be RFC3339 timestamp or YYYY-MM-DD",
        field
    )))
}

async fn ensure_project_membership(
    pool: &SqlitePool,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<()> {
    let match_id = uuid_sql::match_uuid_clause("p.id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let member_match = uuid_sql::match_uuid_clause("pm.user_id");
    let project_case = uuid_sql::case_uuid("p.id");
    let sql = format!(
        "SELECT {} FROM projects p
         WHERE {} AND p.deleted_at IS NULL
           AND (
               {}
               OR EXISTS (
                   SELECT 1
                   FROM project_members pm
                   WHERE pm.project_id = p.id
                     AND {}
                     AND pm.deleted_at IS NULL
               )
           )",
        project_case, match_id, match_owner, member_match
    );
    let member_project = sqlx::query_scalar::<_, String>(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;

    let _member_project = member_project.ok_or_else(|| AppError::not_found("project not found"))?;

    Ok(())
}

async fn fetch_task(
    pool: &SqlitePool,
    user_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> AppResult<DbTask> {
    let match_task = uuid_sql::match_uuid_clause("t.id");
    let match_proj = uuid_sql::match_uuid_clause("t.project_id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let member_match = uuid_sql::match_uuid_clause("pm.user_id");

    let sql = format!(
        "SELECT {} \
         FROM tasks t
         INNER JOIN projects p ON p.id = t.project_id
         WHERE {} AND {} AND p.deleted_at IS NULL AND t.deleted_at IS NULL
          AND (
               {}
               OR EXISTS (
                   SELECT 1
                   FROM project_members pm
                   WHERE pm.project_id = p.id
                     AND {}
                     AND pm.deleted_at IS NULL
               )
           )",
        task_metrics::task_select_columns("t"),
        match_task,
        match_proj,
        match_owner,
        member_match
    );

    let row = sqlx::query(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;

    if let Some(r) = row {
        Ok(row_parsers::db_task_from_row(&r)?)
    } else {
        Err(AppError::not_found("task not found"))
    }
}
