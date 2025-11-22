use axum::extract::{Path, State, Query};
use serde::Deserialize;
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::db::{uuid_sql, row_parsers};

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

    let simple = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE task_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(task_id)
    .fetch_all(&state.pool)
    .await;

    let rows = match simple {
        Ok(r) => r,
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
    };

    let items = rows.into_iter().map(Progress::try_from).collect::<Result<_, _>>()?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ProgressFilter {
    pub task_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/progress",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List progress entries", body = [Progress]))
)]
#[allow(dead_code)]
pub async fn list_project_progress(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<ProgressFilter>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Progress>>> {
    // verify project belongs to user
    let owner = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM projects WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;

    let owner = owner.ok_or_else(|| AppError::not_found("project not found"))?;
    if owner != auth.user_id {
        return Err(AppError::forbidden("not allowed to access this project"));
    }

    let rows = if let Some(task_id) = filter.task_id {
        // ensure task belongs to project
        let t_owner = sqlx::query_scalar::<_, Uuid>(
            "SELECT p.user_id FROM projects p INNER JOIN tasks t ON t.project_id = p.id WHERE p.id = ? AND t.id = ? AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
        )
        .bind(project_id)
        .bind(task_id)
        .fetch_optional(&state.pool)
        .await?;

        let t_owner = t_owner.ok_or_else(|| AppError::not_found("task not found"))?;
        if t_owner != auth.user_id {
            return Err(AppError::forbidden("not allowed to access this task"));
        }

        let simple = sqlx::query_as::<_, DbProgress>(
            "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE task_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(&state.pool)
        .await;

        match simple {
            Ok(r) => r,
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
            Ok(r) => r,
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
    };

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

    let simple = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await;

    let row = match simple {
        Ok(r) => r,
        Err(_) => {
            let fallback = sqlx::query(
                "SELECT \
                   CASE WHEN typeof(id)='blob' THEN lower(substr(hex(id),1,8) || '-' || substr(hex(id),9,4) || '-' || substr(hex(id),13,4) || '-' || substr(hex(id),17,4) || '-' || substr(hex(id),21)) ELSE id END as id, \
                   CASE WHEN typeof(project_id)='blob' THEN lower(substr(hex(project_id),1,8) || '-' || substr(hex(project_id),9,4) || '-' || substr(hex(project_id),13,4) || '-' || substr(hex(project_id),17,4) || '-' || substr(hex(project_id),21)) ELSE project_id END as project_id, \
                   CASE WHEN typeof(task_id)='blob' THEN lower(substr(hex(task_id),1,8) || '-' || substr(hex(task_id),9,4) || '-' || substr(hex(task_id),13,4) || '-' || substr(hex(task_id),17,4) || '-' || substr(hex(task_id),21)) ELSE task_id END as task_id, \
                   progress, note, created_at, updated_at, deleted_at \
                 FROM task_progress WHERE ((typeof(id)='blob' AND hex(id)=upper(replace(?,'-',''))) OR (typeof(id)='text' AND id = ?))",
            )
            .bind(id.to_string())
            .bind(id.to_string())
            .fetch_optional(&state.pool)
            .await?;

            let row = fallback.ok_or_else(|| AppError::not_found("progress entry not found"))?;

                row_parsers::db_progress_from_row(&row)?
        }
    };

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

    let simple = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE id = ? AND task_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(task_id)
    .fetch_optional(&state.pool)
    .await?;

    let mut row = match simple {
        Some(r) => r,
        None => {
            // try fallback selecting textified UUIDs
            let fallback = sqlx::query(
                "SELECT \
                   CASE WHEN typeof(id)='blob' THEN lower(substr(hex(id),1,8) || '-' || substr(hex(id),9,4) || '-' || substr(hex(id),13,4) || '-' || substr(hex(id),17,4) || '-' || substr(hex(id),21)) ELSE id END as id, \
                   CASE WHEN typeof(project_id)='blob' THEN lower(substr(hex(project_id),1,8) || '-' || substr(hex(project_id),9,4) || '-' || substr(hex(project_id),13,4) || '-' || substr(hex(project_id),17,4) || '-' || substr(hex(project_id),21)) ELSE project_id END as project_id, \
                   CASE WHEN typeof(task_id)='blob' THEN lower(substr(hex(task_id),1,8) || '-' || substr(hex(task_id),9,4) || '-' || substr(hex(task_id),13,4) || '-' || substr(hex(task_id),17,4) || '-' || substr(hex(task_id),21)) ELSE task_id END as task_id, \
                   progress, note, created_at, updated_at, deleted_at \
                 FROM task_progress WHERE ((typeof(id)='blob' AND hex(id)=upper(replace(?,'-',''))) OR (typeof(id)='text' AND id = ?)) AND task_id = ? AND deleted_at IS NULL",
            )
            .bind(id.to_string())
            .bind(id.to_string())
            .bind(task_id.to_string())
            .fetch_optional(&state.pool)
            .await?;

            let row = fallback.ok_or_else(|| AppError::not_found("progress entry not found"))?;
            row_parsers::db_progress_from_row(&row)?
        }
    };

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

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{task_id}/progress/{id}",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id"), ("id" = Uuid, Path, description = "Progress id")),
    responses((status = 200, description = "Progress detail", body = Progress))
)]
pub async fn get_progress(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<Json<Progress>> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let simple = sqlx::query_as::<_, DbProgress>(
        "SELECT id, project_id, task_id, progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE id = ? AND task_id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(task_id)
    .fetch_optional(&state.pool)
    .await?;

    let row = match simple {
        Some(r) => r,
        None => {
            let fallback = sqlx::query(
                "SELECT \
                   CASE WHEN typeof(id)='blob' THEN lower(substr(hex(id),1,8) || '-' || substr(hex(id),9,4) || '-' || substr(hex(id),13,4) || '-' || substr(hex(id),17,4) || '-' || substr(hex(id),21)) ELSE id END as id, \
                   CASE WHEN typeof(project_id)='blob' THEN lower(substr(hex(project_id),1,8) || '-' || substr(hex(project_id),9,4) || '-' || substr(hex(project_id),13,4) || '-' || substr(hex(project_id),17,4) || '-' || substr(hex(project_id),21)) ELSE project_id END as project_id, \
                   CASE WHEN typeof(task_id)='blob' THEN lower(substr(hex(task_id),1,8) || '-' || substr(hex(task_id),9,4) || '-' || substr(hex(task_id),13,4) || '-' || substr(hex(task_id),17,4) || '-' || substr(hex(task_id),21)) ELSE task_id END as task_id, \
                   progress, note, created_at, updated_at, deleted_at \
                 FROM task_progress WHERE ((typeof(id)='blob' AND hex(id)=upper(replace(?,'-',''))) OR (typeof(id)='text' AND id = ?)) AND task_id = ? AND deleted_at IS NULL",
            )
            .bind(id.to_string())
            .bind(id.to_string())
            .bind(task_id.to_string())
            .fetch_optional(&state.pool)
            .await?;

            let row = fallback.ok_or_else(|| AppError::not_found("progress entry not found"))?;
            row_parsers::db_progress_from_row(&row)?
        }
    };

    let item: Progress = row.try_into()?;
    Ok(Json(item))
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
