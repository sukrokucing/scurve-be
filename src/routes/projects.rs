use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::project::{DbProject, Project, ProjectCreateRequest, ProjectUpdateRequest};
use crate::utils::utc_now;

const DEFAULT_THEME: &str = "#3498db";

#[utoipa::path(
    get,
    path = "/projects",
    tag = "Projects",
    responses((status = 200, description = "List projects", body = [Project]))
)]
pub async fn list_projects(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<Vec<Project>>> {
    let projects = sqlx::query_as::<_, DbProject>(
        "SELECT id, user_id, name, description, theme_color, created_at, updated_at, deleted_at FROM projects WHERE user_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;

    let projects: Vec<Project> = projects
        .into_iter()
        .map(Project::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(projects))
}

#[utoipa::path(
    post,
    path = "/projects",
    tag = "Projects",
    request_body = ProjectCreateRequest,
    responses((status = 201, description = "Project created", body = Project))
)]
pub async fn create_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<ProjectCreateRequest>,
) -> AppResult<(StatusCode, Json<Project>)> {
    let now = utc_now();
    let project_id = Uuid::new_v4();
    let theme_color = payload.theme_color.clone().unwrap_or_else(|| DEFAULT_THEME.to_string());

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(project_id)
    .bind(auth.user_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&theme_color)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let project = fetch_project(&state.pool, auth.user_id, project_id).await?;
    let project: Project = project.try_into()?;

    Ok((StatusCode::CREATED, Json(project)))
}

#[utoipa::path(
    get,
    path = "/projects/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Project detail", body = Project))
)]
pub async fn get_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Project>> {
    let project = fetch_project(&state.pool, auth.user_id, id).await?;
    let project: Project = project.try_into()?;
    Ok(Json(project))
}

#[utoipa::path(
    put,
    path = "/projects/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    request_body = ProjectUpdateRequest,
    responses((status = 200, description = "Project updated", body = Project))
)]
pub async fn update_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProjectUpdateRequest>,
) -> AppResult<Json<Project>> {
    let mut project = fetch_project(&state.pool, auth.user_id, id).await?;

    if let Some(name) = payload.name.as_ref() {
        project.name = name.clone();
    }
    if payload.description.is_some() {
        project.description = payload.description.clone();
    }
    if let Some(theme_color) = payload.theme_color.as_ref() {
        project.theme_color = theme_color.clone();
    }

    let now = utc_now();

    sqlx::query(
        "UPDATE projects SET name = ?, description = ?, theme_color = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&project.name)
    .bind(&project.description)
    .bind(&project.theme_color)
    .bind(now)
    .bind(project.id)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await?;

    project.updated_at = now;
    let project: Project = project.try_into()?;

    Ok(Json(project))
}

#[utoipa::path(
    delete,
    path = "/projects/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 204, description = "Project soft deleted"))
)]
pub async fn delete_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // Ensure project exists and belongs to user
    let _ = fetch_project(&state.pool, auth.user_id, id).await?;

    let now = utc_now();
    let affected = sqlx::query("UPDATE projects SET deleted_at = ?, updated_at = ? WHERE id = ? AND user_id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("project not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_project(pool: &SqlitePool, user_id: Uuid, project_id: Uuid) -> AppResult<DbProject> {
    sqlx::query_as::<_, DbProject>(
        "SELECT id, user_id, name, description, theme_color, created_at, updated_at, deleted_at FROM projects WHERE id = ? AND user_id = ? AND deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("project not found"))
}
