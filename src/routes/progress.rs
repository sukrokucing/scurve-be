use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::progress::{DbProgress, Progress, ProgressCreateRequest, ProgressUpdateRequest};
use crate::utils::utc_now;

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{task_id}/progress",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id")),
    responses((status = 200, description = "List progress entries", body = [Progress]))
)]
pub async fn list_progress(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Progress>>> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let rows = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE task_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(task_id)
    .fetch_all(&state.pool)
    .await?;

    let items = rows.into_iter().map(Progress::try_from).collect::<Result<_, _>>()?;
    Ok(Json(items))
}

#[utoipa::path(
    post,
    path = "/projects/{project_id}/tasks/{task_id}/progress",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id")),
    request_body = ProgressCreateRequest,
    responses((status = 201, description = "Progress created", body = Progress))
)]
pub async fn create_progress(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
    Json(payload): Json<ProgressCreateRequest>,
) -> AppResult<(StatusCode, Json<Progress>)> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    if payload.progress < 0 || payload.progress > 100 {
        return Err(AppError::bad_request("progress must be between 0 and 100"));
    }

    let id = Uuid::new_v4();
    let now = utc_now();

    sqlx::query(
        "INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(task_id)
    .bind(project_id)
    .bind(payload.progress)
    .bind(payload.note)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let item: Progress = row.try_into()?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/{task_id}/progress/{id}",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id"), ("id" = Uuid, Path, description = "Progress id")),
    request_body = ProgressUpdateRequest,
    responses((status = 200, description = "Progress updated", body = Progress))
)]
pub async fn update_progress(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
    Json(payload): Json<ProgressUpdateRequest>,
) -> AppResult<Json<Progress>> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let mut row = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE id = ? AND task_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(task_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("progress entry not found"))?;

    if let Some(p) = payload.progress {
        if p < 0 || p > 100 {
            return Err(AppError::bad_request("progress must be between 0 and 100"));
        }
        row.progress = p;
    }
    if let Some(note) = payload.note {
        row.note = Some(note);
    }

    let now = utc_now();
    // clone optional note so we don't move out of `row` before converting
    let note_val = row.note.clone();
    let id_val = row.id;
    sqlx::query("UPDATE task_progress SET progress = ?, note = ?, updated_at = ? WHERE id = ?")
        .bind(row.progress)
        .bind(note_val)
        .bind(now)
        .bind(id_val)
        .execute(&state.pool)
        .await?;

    row.updated_at = now;
    let item: Progress = row.try_into()?;
    Ok(Json(item))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/tasks/{task_id}/progress/{id}",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id"), ("id" = Uuid, Path, description = "Progress id")),
    responses((status = 204, description = "Progress soft deleted"))
)]
pub async fn delete_progress(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<StatusCode> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let now = utc_now();
    let affected = sqlx::query("UPDATE task_progress SET deleted_at = ?, updated_at = ? WHERE id = ? AND task_id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .bind(task_id)
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("progress entry not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn ensure_task_belongs_to_user(pool: &SqlitePool, user_id: Uuid, project_id: Uuid, task_id: Uuid) -> AppResult<()> {
    let owner = sqlx::query_scalar::<_, Uuid>(
        "SELECT p.user_id FROM projects p INNER JOIN tasks t ON t.project_id = p.id WHERE p.id = ? AND t.id = ? AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    let owner = owner.ok_or_else(|| AppError::not_found("task or project not found"))?;
    if owner != user_id {
        return Err(AppError::forbidden("not allowed to access this task"));
    }
    Ok(())
}
