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
use crate::models::progress::{Progress, ProgressCreateRequest, ProgressUpdateRequest};
use crate::utils::utc_now;

#[utoipa::path(
    get,
    path = "/projects/{project_id}/tasks/{task_id}/progress",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id")),
    responses((status = 200, description = "List progress entries", body = [Progress])),
    security(("bearerAuth" = []))
)]
pub async fn list_progress(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Progress>>> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let id_case = uuid_sql::case_uuid("id");
    let project_case = uuid_sql::case_uuid("project_id");
    let task_case = uuid_sql::case_uuid("task_id");
    let match_task = uuid_sql::match_uuid_clause("task_id");

    let sql = format!(
        "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {} AND deleted_at IS NULL ORDER BY created_at DESC",
        id_case, project_case, task_case, match_task
    );

    let rows = sqlx::query(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        parsed.push(row_parsers::db_progress_from_row(&row)?);
    }

    let items = parsed.into_iter().map(Progress::try_from).collect::<Result<_, _>>()?;
    Ok(Json(items))
}

#[utoipa::path(
    get,
    path = "/tasks/{task_id}/progress",
    tag = "Progress",
    params(("task_id" = Uuid, Path, description = "Task id")),
    responses((status = 200, description = "List progress entries by task id", body = [Progress])),
    security(("bearerAuth" = []))
)]
pub async fn list_progress_by_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Progress>>> {
    ensure_task_belongs_to_user_by_task_id(&state.pool, auth.user_id, task_id).await?;

    let id_case = uuid_sql::case_uuid("id");
    let project_case = uuid_sql::case_uuid("project_id");
    let task_case = uuid_sql::case_uuid("task_id");
    let match_task = uuid_sql::match_uuid_clause("task_id");

    let sql = format!(
        "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {} AND deleted_at IS NULL ORDER BY created_at DESC",
        id_case, project_case, task_case, match_task
    );

    let rows = sqlx::query(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        parsed.push(row_parsers::db_progress_from_row(&row)?);
    }

    let items = parsed.into_iter().map(Progress::try_from).collect::<Result<_, _>>()?;
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
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("task_id" = Option<Uuid>, Query, description = "Optional task id filter.")
    ),
    responses((status = 200, description = "List progress entries", body = [Progress])),
    security(("bearerAuth" = []))
)]
#[allow(dead_code)]
pub async fn list_project_progress(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<ProgressFilter>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Progress>>> {
    // verify project belongs to user
    let match_proj = uuid_sql::match_uuid_clause("id");
    let match_user = uuid_sql::match_uuid_clause("user_id");
    let user_case = uuid_sql::case_uuid("user_id");
    let sql_owner = format!("SELECT {} FROM projects WHERE {} AND {} AND deleted_at IS NULL", user_case, match_proj, match_user);
    let owner_s = sqlx::query_scalar::<_, String>(&sql_owner)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    let _owner = owner_s.ok_or_else(|| AppError::not_found("project not found"))?;

    let id_case = uuid_sql::case_uuid("id");
    let project_case = uuid_sql::case_uuid("project_id");
    let task_case = uuid_sql::case_uuid("task_id");

    let rows = if let Some(task_id) = filter.task_id {
        // ensure task belongs to project
        let p_match = uuid_sql::match_uuid_clause("p.id");
        let t_match = uuid_sql::match_uuid_clause("t.id");
        let p_user_match = uuid_sql::match_uuid_clause("p.user_id");
        let user_case = uuid_sql::case_uuid("p.user_id");
        let sql_t_owner = format!(
            "SELECT {} FROM projects p INNER JOIN tasks t ON t.project_id = p.id WHERE {} AND {} AND {} AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
            user_case, p_match, t_match, p_user_match
        );
        let _t_owner = sqlx::query_scalar::<_, String>(&sql_t_owner)
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .bind(task_id.to_string())
            .bind(task_id.to_string())
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| AppError::not_found("task not found"))?;

        let match_task = uuid_sql::match_uuid_clause("task_id");
        let sql = format!(
            "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {} AND deleted_at IS NULL ORDER BY created_at DESC",
            id_case, project_case, task_case, match_task
        );

        let rows = sqlx::query(&sql)
            .bind(task_id.to_string())
            .bind(task_id.to_string())
            .fetch_all(&state.pool)
            .await?;

        let mut parsed = Vec::with_capacity(rows.len());
        for row in rows {
            parsed.push(row_parsers::db_progress_from_row(&row)?);
        }
        parsed
    } else {
        let match_proj_tp = uuid_sql::match_uuid_clause("project_id");
        let sql = format!(
            "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {} AND deleted_at IS NULL ORDER BY created_at DESC",
            id_case, project_case, task_case, match_proj_tp
        );

        let rows = sqlx::query(&sql)
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .fetch_all(&state.pool)
            .await?;

        let mut parsed = Vec::with_capacity(rows.len());
        for row in rows {
            parsed.push(row_parsers::db_progress_from_row(&row)?);
        }
        parsed
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
    responses((status = 201, description = "Progress created", body = Progress)),
    security(("bearerAuth" = []))
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

    let match_task = uuid_sql::match_uuid_clause("id");
    let match_project = uuid_sql::match_uuid_clause("id");
    let insert_sql = format!(
        "INSERT INTO task_progress (id, task_id, project_id, progress, note, created_at, updated_at) \
         VALUES (?, (SELECT id FROM tasks WHERE {}), (SELECT id FROM projects WHERE {}), ?, ?, ?, ?)",
        match_task, match_project
    );

    sqlx::query(&insert_sql)
    .bind(id.to_string())
    .bind(task_id.to_string())
    .bind(task_id.to_string())
    .bind(project_id.to_string())
    .bind(project_id.to_string())
    .bind(payload.progress)
    .bind(payload.note)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let id_case = uuid_sql::case_uuid("id");
    let project_case = uuid_sql::case_uuid("project_id");
    let task_case = uuid_sql::case_uuid("task_id");
    let match_id = uuid_sql::match_uuid_clause("id");

    let sql = format!(
        "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {}",
        id_case, project_case, task_case, match_id
    );

    let row = sqlx::query(&sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("progress entry not found"))?;

    let parsed = row_parsers::db_progress_from_row(&row)?;
    let item: Progress = parsed.try_into()?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(
    put,
    path = "/projects/{project_id}/tasks/{task_id}/progress/{id}",
    tag = "Progress",
    params(("project_id" = Uuid, Path, description = "Project id"), ("task_id" = Uuid, Path, description = "Task id"), ("id" = Uuid, Path, description = "Progress id")),
    request_body = ProgressUpdateRequest,
    responses((status = 200, description = "Progress updated", body = Progress)),
    security(("bearerAuth" = []))
)]
pub async fn update_progress(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
    Json(payload): Json<ProgressUpdateRequest>,
) -> AppResult<Json<Progress>> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let id_case = uuid_sql::case_uuid("id");
    let project_case = uuid_sql::case_uuid("project_id");
    let task_case = uuid_sql::case_uuid("task_id");
    let match_id = uuid_sql::match_uuid_clause("id");
    let match_task = uuid_sql::match_uuid_clause("task_id");

    let sql = format!(
        "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {} AND {} AND deleted_at IS NULL",
        id_case, project_case, task_case, match_id, match_task
    );

    let row = sqlx::query(&sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("progress entry not found"))?;

    let mut row = row_parsers::db_progress_from_row(&row)?;

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
    responses((status = 204, description = "Progress soft deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_progress(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<StatusCode> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    let match_task = uuid_sql::match_uuid_clause("task_id");
    let sql = format!("UPDATE task_progress SET deleted_at = ?, updated_at = ? WHERE {} AND {} AND deleted_at IS NULL", match_id, match_task);

    let affected = sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .bind(id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
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
    responses((status = 200, description = "Progress detail", body = Progress)),
    security(("bearerAuth" = []))
)]
pub async fn get_progress(
    State(state): State<AppState>,
    Path((project_id, task_id, id)): Path<(Uuid, Uuid, Uuid)>,
    auth: AuthUser,
) -> AppResult<Json<Progress>> {
    ensure_task_belongs_to_user(&state.pool, auth.user_id, project_id, task_id).await?;

    let id_case = uuid_sql::case_uuid("id");
    let project_case = uuid_sql::case_uuid("project_id");
    let task_case = uuid_sql::case_uuid("task_id");
    let match_id = uuid_sql::match_uuid_clause("id");
    let match_task = uuid_sql::match_uuid_clause("task_id");

    let sql = format!(
        "SELECT {} , {} , {} , progress, note, created_at, updated_at, deleted_at FROM task_progress WHERE {} AND {} AND deleted_at IS NULL",
        id_case, project_case, task_case, match_id, match_task
    );

    let row = sqlx::query(&sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("progress entry not found"))?;

    let parsed = row_parsers::db_progress_from_row(&row)?;
    let item: Progress = parsed.try_into()?;
    Ok(Json(item))
}

async fn ensure_task_belongs_to_user(pool: &SqlitePool, user_id: Uuid, project_id: Uuid, task_id: Uuid) -> AppResult<()> {
    let match_proj = uuid_sql::match_uuid_clause("p.id");
    let match_task = uuid_sql::match_uuid_clause("t.id");
    let match_user = uuid_sql::match_uuid_clause("p.user_id");
    let sql = format!(
        "SELECT p.user_id FROM projects p INNER JOIN tasks t ON t.project_id = p.id WHERE {} AND {} AND {} AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
        match_proj, match_task, match_user
    );

    let owner_s = sqlx::query_scalar::<_, String>(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;

    let _owner = owner_s.ok_or_else(|| AppError::not_found("task or project not found"))?;
    Ok(())
}

async fn ensure_task_belongs_to_user_by_task_id(
    pool: &SqlitePool,
    user_id: Uuid,
    task_id: Uuid,
) -> AppResult<()> {
    let match_task = uuid_sql::match_uuid_clause("t.id");
    let match_user = uuid_sql::match_uuid_clause("p.user_id");
    let sql = format!(
        "SELECT p.user_id
         FROM projects p
         INNER JOIN tasks t ON t.project_id = p.id
         WHERE {} AND {} AND p.deleted_at IS NULL AND t.deleted_at IS NULL",
        match_task, match_user
    );

    let owner_s = sqlx::query_scalar::<_, String>(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;

    let _owner = owner_s.ok_or_else(|| AppError::not_found("task not found"))?;
    Ok(())
}
