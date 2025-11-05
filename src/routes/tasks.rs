use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::task::{DbTask, Task, TaskCreateRequest, TaskUpdateRequest};
use crate::utils::utc_now;

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
    auth: AuthUser,
) -> AppResult<Json<Vec<Task>>> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let tasks = sqlx::query_as::<_, DbTask>(
        "SELECT t.id, t.project_id, t.title, t.status, t.due_date, t.created_at, t.updated_at, t.deleted_at
         FROM tasks t
         WHERE t.project_id = ? AND t.deleted_at IS NULL
         ORDER BY t.created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;

    let tasks: Vec<Task> = tasks
        .into_iter()
        .map(Task::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(tasks))
}

#[utoipa::path(
    post,
    path = "/tasks",
    tag = "Tasks",
    request_body = TaskCreateRequest,
    responses((status = 201, description = "Task created", body = Task))
)]
pub async fn create_task(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    auth: AuthUser,
    Json(payload): Json<TaskCreateRequest>,
) -> AppResult<(StatusCode, Json<Task>)> {
    ensure_project_membership(&state.pool, auth.user_id, project_id).await?;

    let task_id = Uuid::new_v4();
    let now = utc_now();
    let status = payload.status.clone().unwrap_or_else(|| "pending".to_string());

    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, status, due_date, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(&payload.title)
    .bind(status)
    .bind(payload.due_date)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let task = fetch_task(&state.pool, auth.user_id, project_id, task_id).await?;
    let task: Task = task.try_into()?;

    Ok((StatusCode::CREATED, Json(task)))
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
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<TaskUpdateRequest>,
) -> AppResult<Json<Task>> {
    let mut task = fetch_task(&state.pool, auth.user_id, project_id, id).await?;

    let TaskUpdateRequest {
        title,
        status,
        due_date,
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

    let now = utc_now();

    sqlx::query(
        "UPDATE tasks SET title = ?, status = ?, due_date = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&task.title)
    .bind(&task.status)
    .bind(task.due_date)
    .bind(now)
    .bind(task.id)
    .execute(&state.pool)
    .await?;

    task.updated_at = now;
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
    sqlx::query_as::<_, DbTask>(
        "SELECT t.id, t.project_id, t.title, t.status, t.due_date, t.created_at, t.updated_at, t.deleted_at
         FROM tasks t
         INNER JOIN projects p ON p.id = t.project_id
         WHERE t.id = ? AND t.project_id = ? AND p.user_id = ? AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("task not found"))
}
