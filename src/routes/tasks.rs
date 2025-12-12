use axum::extract::{Path, State, Query};
use chrono::Utc;
use serde::Deserialize;
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::db::{uuid_sql, row_parsers};

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::task::{DbTask, Task, TaskCreateRequest, TaskUpdateRequest};
use crate::models::dependency::{TaskDependency, DependencyCreateRequest};
use crate::models::progress::DbProgress;
use crate::utils::{utc_now, normalize_to_midnight};

#[derive(Debug, Deserialize)]
pub struct TaskListQuery {
    pub progress: Option<bool>,
    pub task_id: Option<Uuid>,
}
#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List tasks", body = [Task]))
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TaskListQuery>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Task>>> {
    // If caller requested progress via query param, return progress entries instead
    if query.progress.unwrap_or(false) {
        // verify project membership
        ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let _rows = if let Some(task_id) = query.task_id {
            // ensure task belongs to project
            let _ = fetch_task(&state.pool, auth.user_id, project_id, task_id).await?;
            let simple = sqlx::query_as::<_, DbProgress>(
                "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE task_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
            )
            .bind(task_id)
            .fetch_all(&state.pool)
            .await;

            match simple {
                Ok(rows) => rows,
                Err(_) => {
                            let id_case = uuid_sql::case_uuid("id");
                            let project_case = uuid_sql::case_uuid("project_id");
                            let task_case = uuid_sql::case_uuid("task_id");
                            let sql = format!(
                                "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE task_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
                                id_case, project_case, task_case
                            );

                            let rows = sqlx::query(&sql)
                                .bind(task_id.to_string())
                                .fetch_all(&state.pool)
                                .await?;

                    let mut parsed = Vec::with_capacity(rows.len());
                    for row in rows {
                        parsed.push(row_parsers::db_progress_from_row(&row)?);
                    }

                    parsed
                }
            }
        } else {
            let simple = sqlx::query_as::<_, DbProgress>(
                "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE project_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
            )
            .bind(project_id)
            .fetch_all(&state.pool)
            .await;

            match simple {
                Ok(rows) => rows,
                Err(_) => {
                    let id_case = uuid_sql::case_uuid("id");
                    let project_case = uuid_sql::case_uuid("project_id");
                    let task_case = uuid_sql::case_uuid("task_id");
                    let sql = format!(
                        "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE project_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
                        id_case, project_case, task_case
                    );

                    let rows = sqlx::query(&sql)
                        .bind(project_id.to_string())
                        .fetch_all(&state.pool)
                        .await?;

                    let mut parsed = Vec::with_capacity(rows.len());
                    for row in rows {
                        parsed.push(row_parsers::db_progress_from_row(&row)?);
                    }

                    parsed
                }
            }
        }
    ;

        // Convert to Progress and then to Task-like JSON via serde Value? We will return empty Vec<Task> to satisfy signature
        // But to avoid breaking the signature, we'll return an empty task list when progress=true — caller should use the progress endpoints.
        // For now, return an empty Vec<Task> as placeholder.
        let tasks: Vec<Task> = Vec::new();
        return Ok(Json(tasks));
    }

    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;


    // Try simple fast-path query first
    let simple = sqlx::query_as::<_, DbTask>(
        "SELECT t.id, t.project_id, t.title, t.status, t.due_date, t.start_date, t.end_date, t.duration_days, t.assignee, t.parent_id, t.progress, t.created_at, t.updated_at, t.deleted_at
         FROM tasks t
         WHERE t.project_id = ? AND t.deleted_at IS NULL
         ORDER BY t.start_date ASC, t.created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await;

    let tasks_rows: Vec<DbTask> = match simple {
        Ok(rows) => rows,
        Err(_) => {
            // Fallback: select textified UUIDs and parse manually
            let id_case = uuid_sql::case_uuid("id");
            let project_case = uuid_sql::case_uuid("project_id");
            let assignee_case = uuid_sql::case_uuid("assignee");
            let parent_case = uuid_sql::case_uuid("parent_id");
            let sql = format!(
                "SELECT {} , {} , title, status, due_date, start_date, end_date, duration_days, {} , {} , progress, created_at, updated_at, deleted_at FROM tasks t WHERE t.project_id = ? AND t.deleted_at IS NULL ORDER BY t.start_date ASC, t.created_at DESC",
                id_case, project_case, assignee_case, parent_case
            );

            let rows = sqlx::query(&sql)
                .bind(project_id.to_string())
                .fetch_all(&state.pool)
                .await?;

                    let mut parsed = Vec::with_capacity(rows.len());
            for row in rows {
                parsed.push(row_parsers::db_task_from_row(&row)?);
            }

            parsed
        }
    };

    let tasks: Vec<Task> = tasks_rows
        .into_iter()
        .map(Task::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(tasks))
}

#[utoipa::path(
    post,
    path = "/projects/{project_id}/tasks",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = TaskCreateRequest,
    responses((status = 201, description = "Task created", body = Task))
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
    let status = payload.status.clone().unwrap_or_else(|| "pending".to_string());

    // Normalize dates to midnight UTC for consistent milestone detection
    let start_date = payload.start_date.map(normalize_to_midnight);
    let end_date = payload.end_date.map(normalize_to_midnight);

    // Validate timeline fields
    if let (Some(start), Some(end)) = (start_date, end_date) {
        if end < start {
            return Err(AppError::bad_request("end_date must be >= start_date"));
        }
    }

    if let Some(p) = payload.progress {
        if p < 0 || p > 100 {
            return Err(AppError::bad_request("progress must be between 0 and 100"));
        }
    }

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, due_date, start_date, end_date, assignee, parent_id, progress, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(&payload.title)
    .bind(status)
    .bind(payload.due_date)
    .bind(start_date)
    .bind(end_date)

    .bind(payload.assignee)
    .bind(payload.parent_id)
    // default progress to 0 when not provided
    .bind(payload.progress.unwrap_or(0))
    .bind(now)
    .bind(now)
    // ... [existing insert logic]
    .execute(&state.pool)
    .await?;

    let task = fetch_task(&state.pool, auth.user_id, project_id, task_id).await?;
    let task_dto: Task = task.clone().try_into()?;

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
    responses((status = 200, description = "Task updated", body = Task))
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
    let old_dto: Task = old_task.clone().try_into()?;

    let mut task = old_task;

    let TaskUpdateRequest {
        title,
        status,
        due_date,
        start_date,
        end_date,
        assignee,
        parent_id,
        progress,
    } = payload;

    if let Some(title) = title {
        task.title = title;
    }
    if let Some(status) = status {
        task.status = status;
    }
    if let Some(due_date) = due_date {
        task.due_date = Some(due_date);
    }

    if let Some(sd) = start_date {
        task.start_date = Some(normalize_to_midnight(sd));
    }
    if let Some(ed) = end_date {
        task.end_date = Some(normalize_to_midnight(ed));
    }
    if let Some(a) = assignee {
        task.assignee = Some(a);
    }
    if let Some(pid) = parent_id {
        task.parent_id = Some(pid);
    }
    if let Some(p) = progress {
        if p < 0 || p > 100 {
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

    let now = utc_now();

    sqlx::query(
        "UPDATE tasks SET title = ?, status = ?, due_date = ?, start_date = ?, end_date = ?, assignee = ?, parent_id = ?, progress = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&task.title)
    .bind(&task.status)
    .bind(task.due_date)
    .bind(task.start_date)
    .bind(task.end_date)
    .bind(task.assignee)
    .bind(task.parent_id)
    .bind(task.progress)
    .bind(now)
    .bind(task.id)
    .execute(&state.pool)
    .await?;

    // Re-fetch to get the DB-calculated fields (like duration_days from triggers)
    let task = fetch_task(&state.pool, auth.user_id, project_id, task.id).await?;
    let task_dto: Task = task.clone().try_into()?;

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
    responses((status = 200, description = "Task detail", body = Task))
)]
pub async fn get_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Task>> {
    let task = fetch_task(&state.pool, auth.user_id, project_id, id).await?;
    let task: Task = task.try_into()?;
    Ok(Json(task))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/tasks/{id}",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id"), ("id" = Uuid, Path, description = "Task id")),
    responses((status = 204, description = "Task soft deleted"))
)]
pub async fn delete_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let _ = fetch_task(&state.pool, auth.user_id, project_id, id).await?;

    let now = utc_now();
    let affected = sqlx::query("UPDATE tasks SET deleted_at = ?, updated_at = ? WHERE id = ? AND project_id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .bind(project_id)
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("task not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/dependencies",
    tag = "Dependencies",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List dependencies", body = [TaskDependency]))
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
    responses((status = 201, description = "Dependency created", body = TaskDependency))
)]
pub async fn create_dependency(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    auth: AuthUser,
    Json(payload): Json<DependencyCreateRequest>,
) -> AppResult<(StatusCode, Json<TaskDependency>)> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    // Validate tasks exist and belong to project
    let _source = fetch_task(&state.pool, auth.user_id, project_id, payload.source_task_id).await?;
    let _target = fetch_task(&state.pool, auth.user_id, project_id, payload.target_task_id).await?;

    if payload.source_task_id == payload.target_task_id {
        return Err(AppError::bad_request("Cannot link task to itself"));
    }

    // Check for existing reverse link to prevent immediate cycle (A->B and B->A)
    // Note: Deep cycle detection (A->B->C->A) is complex and omitted for MVP as per plan.
    let reverse_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM task_dependencies WHERE source_task_id = ? AND target_task_id = ?)"
    )
    .bind(payload.target_task_id)
    .bind(payload.source_task_id)
    .fetch_one(&state.pool)
    .await?;

    if reverse_exists {
        return Err(AppError::bad_request("Cycle detected: reverse dependency already exists"));
    }

    // Detect deeper cycles (A->B->C->...->A) using a recursive CTE. If there exists
    // a path from the intended target back to the intended source, inserting this
    // dependency would create a cycle.
    let cycle_exists: bool = sqlx::query_scalar(
        "WITH RECURSIVE reach(node) AS (
            SELECT target_task_id FROM task_dependencies WHERE source_task_id = ?
            UNION
            SELECT d.target_task_id FROM task_dependencies d JOIN reach r ON d.source_task_id = r.node
        )
        SELECT EXISTS(SELECT 1 FROM reach WHERE node = ?);"
    )
    .bind(payload.target_task_id)
    .bind(payload.source_task_id)
    .fetch_one(&state.pool)
    .await?;

    if cycle_exists {
        return Err(AppError::bad_request("Cycle detected: would create circular dependency"));
    }

    let id = Uuid::new_v4();
    let now = utc_now();

    sqlx::query(
        "INSERT INTO task_dependencies (id, source_task_id, target_task_id, type, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(id)
    .bind(payload.source_task_id)
    .bind(payload.target_task_id)
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
    responses((status = 204, description = "Dependency deleted"))
)]
pub async fn delete_dependency(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<StatusCode> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    // We need to verify the dependency belongs to a task in this project
    // We can join tasks to verify
    let affected = sqlx::query(
        "DELETE FROM task_dependencies \
         WHERE id = ? AND source_task_id IN (SELECT id FROM tasks WHERE project_id = ?)"
    )
    .bind(id)
    .bind(project_id)
    .execute(&state.pool)
    .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("Dependency not found or not in project"));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/batch",
    tag = "Tasks",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = TaskBatchUpdatePayload,
    responses((status = 200, description = "Tasks updated", body = [Task]))
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
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ? AND project_id = ? AND deleted_at IS NULL)"
        )
        .bind(update.id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;

        if !exists {
            return Err(AppError::not_found(format!("Task {} not found in project", update.id)));
        }

        let current: DbTask = sqlx::query_as(
            "SELECT * FROM tasks WHERE id = ?"
        )
        .bind(update.id)
        .fetch_one(&mut *tx)
        .await?;

        // Normalize dates to midnight UTC and validate timeline if changing
        let start = update.start_date.map(normalize_to_midnight).or(current.start_date.map(|d| d.with_timezone(&Utc)));
        let end = update.end_date.map(normalize_to_midnight).or(current.end_date.map(|d| d.with_timezone(&Utc)));

        if let (Some(s), Some(e)) = (start, end) {
             if e < s {
                return Err(AppError::bad_request(format!("Task {}: end_date must be >= start_date", update.id)));
            }
        }

        if let Some(p) = update.progress {
             if p < 0 || p > 100 {
                return Err(AppError::bad_request(format!("Task {}: progress must be between 0 and 100", update.id)));
            }
        }

        let title = update.title.unwrap_or(current.title);
        let status = update.status.unwrap_or(current.status);
        let due_date = update.due_date.or(current.due_date.map(|d| d.with_timezone(&Utc)));
        let start_date = update.start_date.map(normalize_to_midnight).or(current.start_date.map(|d| d.with_timezone(&Utc)));
        let end_date = update.end_date.map(normalize_to_midnight).or(current.end_date.map(|d| d.with_timezone(&Utc)));
        let assignee = update.assignee.or(current.assignee);
        let parent_id = update.parent_id.or(current.parent_id);
        let progress = update.progress.unwrap_or(current.progress);

        sqlx::query(
            "UPDATE tasks SET title = ?, status = ?, due_date = ?, start_date = ?, end_date = ?, assignee = ?, parent_id = ?, progress = ?, updated_at = ? WHERE id = ?"
        )
        .bind(title)
        .bind(status)
        .bind(due_date)
        .bind(start_date)
        .bind(end_date)
        .bind(assignee)
        .bind(parent_id)
        .bind(progress)
        .bind(now)
        .bind(update.id)
        .execute(&mut *tx)
        .await?;

        updated_ids.push(update.id);
    }

    tx.commit().await?;

    if updated_ids.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let placeholders = std::iter::repeat("?").take(updated_ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT t.id, t.project_id, t.title, t.status, t.due_date, t.start_date, t.end_date, t.duration_days, t.assignee, t.parent_id, t.progress, t.created_at, t.updated_at, t.deleted_at \
         FROM tasks t \
         WHERE t.id IN ({}) ORDER BY t.start_date ASC",
        placeholders
    );

    let mut query = sqlx::query_as::<_, DbTask>(&sql);
    for id in updated_ids {
        query = query.bind(id);
    }

    let rows = query.fetch_all(&state.pool).await?;

    let tasks: Vec<Task> = rows
        .into_iter()
        .map(Task::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(tasks))
}

async fn ensure_project_membership(pool: &SqlitePool, user_id: Uuid, project_id: Uuid) -> AppResult<()> {
    let owner = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM projects WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let owner = owner.ok_or_else(|| AppError::not_found("project not found"))?;

    if owner != user_id {
        return Err(AppError::forbidden("not allowed to modify this project"));
    }

    Ok(())
}

async fn fetch_task(pool: &SqlitePool, user_id: Uuid, project_id: Uuid, task_id: Uuid) -> AppResult<DbTask> {
    // Try simple direct mapping first
    let simple = sqlx::query_as::<_, DbTask>(
        "SELECT t.id, t.project_id, t.title, t.status, t.due_date, t.start_date, t.end_date, t.duration_days, t.assignee, t.parent_id, t.progress, t.created_at, t.updated_at, t.deleted_at
         FROM tasks t
         INNER JOIN projects p ON p.id = t.project_id
         WHERE t.id = ? AND t.project_id = ? AND p.user_id = ? AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await;

    match simple {
        Ok(Some(row)) => Ok(row),
        Ok(None) => Err(AppError::not_found("task not found")),
        Err(_) => {
            // Fallback: select textified UUIDs and parse manually
            let fallback = sqlx::query(
                "SELECT \
                   CASE WHEN typeof(t.id)='blob' THEN lower(substr(hex(t.id),1,8) || '-' || substr(hex(t.id),9,4) || '-' || substr(hex(t.id),13,4) || '-' || substr(hex(t.id),17,4) || '-' || substr(hex(t.id),21)) ELSE t.id END as id, \
                   CASE WHEN typeof(t.project_id)='blob' THEN lower(substr(hex(t.project_id),1,8) || '-' || substr(hex(t.project_id),9,4) || '-' || substr(hex(t.project_id),13,4) || '-' || substr(hex(t.project_id),17,4) || '-' || substr(hex(t.project_id),21)) ELSE t.project_id END as project_id, \
                   t.title, t.status, t.due_date, t.start_date, t.end_date, t.duration_days, \
                   CASE WHEN typeof(t.assignee)='blob' THEN lower(substr(hex(t.assignee),1,8) || '-' || substr(hex(t.assignee),9,4) || '-' || substr(hex(t.assignee),13,4) || '-' || substr(hex(t.assignee),17,4) || '-' || substr(hex(t.assignee),21)) ELSE t.assignee END as assignee, \
                   CASE WHEN typeof(t.parent_id)='blob' THEN lower(substr(hex(t.parent_id),1,8) || '-' || substr(hex(t.parent_id),9,4) || '-' || substr(hex(t.parent_id),13,4) || '-' || substr(hex(t.parent_id),17,4) || '-' || substr(hex(t.parent_id),21)) ELSE t.parent_id END as parent_id, \
                   t.progress, t.created_at, t.updated_at, t.deleted_at \
                 FROM tasks t INNER JOIN projects p ON p.id = t.project_id \
                 WHERE ((typeof(t.id)='blob' AND hex(t.id)=upper(replace(?,'-',''))) OR (typeof(t.id)='text' AND t.id = ?)) \
                   AND ((typeof(t.project_id)='blob' AND hex(t.project_id)=upper(replace(?,'-',''))) OR (typeof(t.project_id)='text' AND t.project_id = ?)) \
                   AND ((typeof(p.user_id)='blob' AND hex(p.user_id)=upper(replace(?,'-',''))) OR (typeof(p.user_id)='text' AND p.user_id = ?)) \
                   AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
            )
            .bind(task_id.to_string())
            .bind(task_id.to_string())
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await?;

            if let Some(row) = fallback {
                return Ok(row_parsers::db_task_from_row(&row)?);
            }

            Err(AppError::not_found("task not found"))
        }
    }
}
